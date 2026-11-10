//! 暂存区：整文件与部分暂存／取消暂存，以及暂存区内容的读取。
//!
//! 提交的内容来源是真实 Git 暂存区（index），与终端里的 `git add`、`git reset` 共享。整文件操作
//! 交给 `git add`、`git restore --staged`；部分操作把目标内容写成只含一个文件的统一差异，交给
//! `git apply --cached` 写入：`git` 按上下文核对暂存区里的内容，选择过期或索引被占用时拒绝写入，
//! 暂存区保持原样。

use std::ops::Range;

use winc_core::diff::{LineHunk, line_hunks, lines};

use super::*;

/// 部分暂存的统一差异在每处改动前后各带几行上下文（行）。
///
/// 上下文用于在暂存区里定位改动，也是 `git apply` 核对内容的依据；上下文对不上时不写入。
const PATCH_CONTEXT: usize = 3;

/// 部分暂存把目标内容写到暂存区还是从暂存区撤回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// 暂存：暂存区里的内容改成目标内容。
    Stage,
    /// 取消暂存：暂存区里的内容改回目标内容。
    Unstage,
}

impl Git {
    /// 暂存整个仓库的改动（在仓库根目录运行 `git add --all`）。
    ///
    /// 范围是当前仓库，不受状态列表的数量上限影响；未跟踪且未被忽略的文件一并加入。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、`git` 报错时返回它的说明。
    pub fn stage_all(&self, root: &Path) -> Result<(), String> {
        self.require_trust(root)?;
        let mut command = self.command(root);
        command.args(["add", "--all"]);
        self.run(&mut command, STATUS_TIMEOUT, &Cancel::default())?;
        Ok(())
    }

    /// 取消暂存整个仓库的改动：暂存区恢复成 `HEAD` 里的内容，工作区不变。
    ///
    /// 范围是当前仓库，不受状态列表的数量上限影响。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、`git` 报错时返回它的说明。
    pub fn unstage_all(&self, root: &Path) -> Result<(), String> {
        self.require_trust(root)?;
        let cancel = Cancel::default();
        let mut command = self.command(root);
        command
            .args(["rev-parse", "--verify", "--quiet", "HEAD"])
            .stderr(Stdio::piped());
        let head = match process::run(&mut command, Vec::new(), SHOW_TIMEOUT, &cancel) {
            Ok(output) if output.status.success() => true,
            Ok(output) if output.status.code() == Some(1) => false,
            Ok(output) => return Err(failure(&output.stderr)),
            Err(e) => return Err(describe(&e)),
        };
        let mut command = self.command(root);
        if head {
            command.args(["restore", "--staged", "--source=HEAD", "--", "."]);
        } else {
            // 还没有提交过：`restore` 找不到 `HEAD`，用 `read-tree` 清空暂存区。
            command.args(["read-tree", "--empty"]);
        }
        self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
        Ok(())
    }

    /// 取消暂存 `paths`：暂存区恢复成 `HEAD` 里的内容，工作区和编辑器里的内容不变。
    ///
    /// `HEAD` 里没有的路径（新加入版本管理的文件）从暂存区移除，重新成为未跟踪文件；还没有提交过
    /// 时同样只移除暂存区里的条目。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、`git` 报错时返回它的说明。
    pub fn unstage(&self, root: &Path, paths: &[PathBuf]) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        self.require_trust(root)?;
        let cancel = Cancel::default();
        // 有 `HEAD` 时按它恢复暂存区；还没有提交过时只移除条目。退出码 1 是没有 `HEAD`，其他失败
        // 不能当作没有提交过：那会把条目直接移出暂存区。
        let mut command = self.command(root);
        command
            .args(["rev-parse", "--verify", "--quiet", "HEAD"])
            .stderr(Stdio::piped());
        let head = match process::run(&mut command, Vec::new(), SHOW_TIMEOUT, &cancel) {
            Ok(output) if output.status.success() => true,
            Ok(output) if output.status.code() == Some(1) => false,
            Ok(output) => return Err(failure(&output.stderr)),
            Err(e) => return Err(describe(&e)),
        };
        let mut command = self.command(root);
        if head {
            command
                .args(["restore", "--staged", "--source=HEAD", "--"])
                .args(relative_all(root, paths));
        } else {
            // 还没有提交过时 `git restore` 找不到 `HEAD`，改为只改动暂存区：`update-index` 收的是
            // 文件名而不是路径匹配规则，按原样的相对路径传。
            command.args(["update-index", "--force-remove", "--"]);
            for path in paths {
                command.arg(slash_path(root, path)?);
            }
        }
        self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
        Ok(())
    }

    /// 返回 `path` 在暂存区里的内容，套用检出时的转换（同 [`Git::content_at`] 的空版本）；不在暂存
    /// 区里时为 `None`。
    ///
    /// # Errors
    ///
    /// `git` 运行失败、超时或被取消时返回原因。
    pub fn index_content(
        &self,
        root: &Path,
        path: &Path,
        cancel: &Cancel,
    ) -> Result<Option<Vec<u8>>, String> {
        self.content_at(root, "", path, cancel)
    }

    /// 暂存 `path` 的一部分：把暂存区里 `path` 的内容改成 `content`。
    ///
    /// `content` 是工作区的形式（与 [`Git::content_at`] 读出的内容相同），由调用方按选中的改动块与
    /// 行组合；`path` 必须是普通文本文件，符号链接、子模块和二进制内容由整文件操作处理。
    ///
    /// 暂存区里没有该文件时新建条目；`content` 为空且文件已从磁盘删除时等同于整文件暂存这次删除。
    /// 写入前 `git` 按上下文核对暂存区里的内容，与补丁对不上时返回原因且不改动暂存区，不会改成整
    /// 文件暂存。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、内容无法安全映射成文本、`git` 报错时返回它的说明。
    pub fn stage_partial(&self, root: &Path, path: &Path, content: &[u8]) -> Result<(), String> {
        self.write_index(root, path, content, Direction::Stage)
    }

    /// 取消暂存 `path` 的一部分：把暂存区里 `path` 的内容改成 `content`，工作区不变。
    ///
    /// `content` 是工作区的形式（与 [`Git::content_at`] 读出的内容相同），由调用方按要保留的改动块
    /// 与行组合；`path` 必须是普通文本文件，符号链接、子模块和二进制内容由整文件操作处理。
    ///
    /// `HEAD` 里没有该文件时，`content` 为空等同于把条目移出暂存区，文件重新成为未跟踪文件。写入前
    /// `git` 按上下文核对暂存区里的内容，与补丁对不上时返回原因且不改动暂存区，不会改成整文件取消
    /// 暂存。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、内容无法安全映射成文本、`git` 报错时返回它的说明。
    pub fn unstage_partial(&self, root: &Path, path: &Path, content: &[u8]) -> Result<(), String> {
        self.write_index(root, path, content, Direction::Unstage)
    }

    /// 把 `path` 在暂存区里的内容改成 `content`，见 [`Git::stage_partial`] 与
    /// [`Git::unstage_partial`]。
    fn write_index(
        &self,
        root: &Path,
        path: &Path,
        content: &[u8],
        direction: Direction,
    ) -> Result<(), String> {
        self.require_trust(root)?;
        let cancel = Cancel::default();
        let relative = slash_path(root, path)?;
        // 空内容在两侧的含义不同：暂存删除文件的结果是暂存区里没有这个条目，取消暂存新文件的结果
        // 同样是移除条目，而不是存一个空文件。
        if content.is_empty() {
            match direction {
                Direction::Stage if !path.exists() => {
                    return self.stage(root, &[path.to_path_buf()]).map(|_| ());
                }
                Direction::Unstage if self.head_content(root, path, &cancel)?.is_none() => {
                    let mut command = self.command(root);
                    command
                        .args(["update-index", "--force-remove", "--"])
                        .arg(&relative);
                    self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
                    return Ok(());
                }
                _ => {}
            }
        }
        let old = self.index_blob(root, &relative, &cancel)?;
        if direction == Direction::Unstage && old.is_none() {
            return Ok(());
        }
        let (oid, new) = self.blob_content(root, &relative, content, &cancel)?;
        if old.as_deref() == Some(new.as_slice()) {
            return Ok(());
        }
        // 新建条目时补丁要写明模式：暂存区里没有别的地方能知道文件是否可执行。
        let mode = if old.is_none() {
            Some(self.file_mode(path, &cancel)?)
        } else {
            None
        };
        let name = crate::display_name(path).into_owned();
        let (before, after) = match direction {
            Direction::Stage => (old.as_deref().unwrap_or_default(), new.as_slice()),
            // 取消暂存写成反向差异：补丁里列出的是要撤回的改动，`--reverse` 按暂存区里的内容核对。
            Direction::Unstage => (new.as_slice(), old.as_deref().unwrap_or_default()),
        };
        let patch = unified_patch(&relative, mode, before, after, &name)?;
        if patch.is_empty() {
            // 空内容的新条目没有可以写成差异的行，直接把内容写进暂存区。
            if let (None, Some(mode)) = (old.as_ref(), mode) {
                let mut command = self.command(root);
                command
                    .args(["update-index", "--add", "--cacheinfo"])
                    .arg(joined_arg(&[
                        OsStr::new(&format!("{mode},{oid},")),
                        &relative,
                    ]));
                self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
            }
            return Ok(());
        }
        let mut command = self.command(root);
        command.args(["apply", "--cached", "--whitespace=nowarn"]);
        if direction == Direction::Unstage {
            command.arg("--reverse");
        }
        command.arg("-").stderr(Stdio::piped());
        match process::run(&mut command, patch, STATUS_TIMEOUT, &cancel) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(failure(&output.stderr)),
            Err(e) => Err(describe(&e)),
        }
    }

    /// 返回 `path` 在暂存区里的原始内容（Git 保存的形式，不套用过滤器）；不在暂存区里时为 `None`。
    ///
    /// 部分暂存的补丁与暂存区逐字节比较，两侧都必须是不套用过滤器的内容。
    fn index_blob(
        &self,
        root: &Path,
        relative: &OsStr,
        cancel: &Cancel,
    ) -> Result<Option<Vec<u8>>, String> {
        let mut command = self.command(root);
        command
            .args(["cat-file", "blob"])
            .arg(joined_arg(&[OsStr::new(":"), relative]))
            // 要按 `git` 的报错认出「不在暂存区里」，报错不能被翻译成用户的语言。
            .env("LC_ALL", "C")
            .stderr(Stdio::piped());
        match process::run(&mut command, Vec::new(), SHOW_TIMEOUT, cancel) {
            Ok(output) if output.status.success() => Ok(Some(output.stdout)),
            Ok(output) => {
                // 冲突文件在暂存区里有多份内容，写不进单份内容：返回原因，不能当成新文件覆盖冲突。
                let text = String::from_utf8_lossy(&output.stderr);
                if output.status.code() == Some(128)
                    && absent(&output.stderr, "")
                    && !text.contains("is in the index, but not at stage")
                {
                    Ok(None)
                } else {
                    Err(failure(&output.stderr))
                }
            }
            Err(e) => Err(describe(&e)),
        }
    }

    /// 返回 `content`（工作区的形式）在 Git 里保存的字节：套用 `git add` 会用的过滤器
    /// （`core.autocrlf` 的换行转换、`.gitattributes` 里配置的过滤器）。返回值前面是对象的提交号。
    ///
    /// 过滤器由 `git` 执行，转换规则与整文件暂存完全一致。写出的对象在补丁被拒绝时不可达，由
    /// `git gc` 回收。
    fn blob_content(
        &self,
        root: &Path,
        relative: &OsStr,
        content: &[u8],
        cancel: &Cancel,
    ) -> Result<(String, Vec<u8>), String> {
        let mut command = self.command(root);
        command
            .args(["hash-object", "-w", "--stdin"])
            .arg(joined_arg(&[OsStr::new("--path="), relative]))
            .stderr(Stdio::piped());
        let oid = match process::run(&mut command, content.to_vec(), SHOW_TIMEOUT, cancel) {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_owned()
            }
            Ok(output) => return Err(failure(&output.stderr)),
            Err(e) => return Err(describe(&e)),
        };
        let mut command = self.command(root);
        command
            .args(["cat-file", "blob", &oid])
            .stderr(Stdio::piped());
        Ok((oid, self.run(&mut command, SHOW_TIMEOUT, cancel)?))
    }
}

/// 返回把 `before` 改成 `after` 的统一差异，只含 `name` 一个文件；`before` 为空且 `mode` 不为空时
/// 是新建文件的差异。
///
/// 每处改动前后各带 [`PATCH_CONTEXT`] 行上下文，相邻的改动并成一块。内容不是 UTF-8 文本或含 NUL
/// 时返回错误：这样的文件无法逐行映射，只能整文件暂存。`shown` 是错误说明里的文件名。
fn unified_patch(
    name: &OsStr,
    mode: Option<&str>,
    before: &[u8],
    after: &[u8],
    shown: &str,
) -> Result<Vec<u8>, String> {
    let (Some(before), Some(after)) = (text(before), text(after)) else {
        return Err(winc_i18n::t!("git-stage-partial-not-utf8", name = shown));
    };
    let hunks = line_hunks(before, after);
    // 只有换行符的写法不同：Git 不把这类差异算作改动，暂存区里的内容已经等价。
    if hunks.is_empty() {
        return Ok(Vec::new());
    }
    let old = lines(before);
    let new = lines(after);
    let mut out = Vec::with_capacity(before.len() + after.len());
    let relative = name.as_encoded_bytes();
    out.extend_from_slice(b"diff --git ");
    out.extend_from_slice(&patch_name("a/", relative));
    out.push(b' ');
    out.extend_from_slice(&patch_name("b/", relative));
    out.push(b'\n');
    if let Some(mode) = mode {
        out.extend_from_slice(format!("new file mode {mode}\n").as_bytes());
        out.extend_from_slice(b"--- /dev/null\n");
        out.extend_from_slice(&patch_name("+++ b/", relative));
    } else {
        out.extend_from_slice(&patch_name("--- a/", relative));
        out.push(b'\n');
        out.extend_from_slice(&patch_name("+++ b/", relative));
    }
    out.push(b'\n');
    for block in blocks(&hunks, old.len()) {
        let count = |range: &Range<usize>| match range.len() {
            0 => format!("{},0", range.start),
            len => format!("{},{}", range.start + 1, len),
        };
        let header = format!("@@ -{} +{} @@\n", count(&block.old), count(&block.new));
        out.extend_from_slice(header.as_bytes());
        let mut at = block.old.start;
        for hunk in &block.hunks {
            for line in &old[at..hunk.old.start] {
                push_line(&mut out, b' ', line);
            }
            for line in &old[hunk.old.clone()] {
                push_line(&mut out, b'-', line);
            }
            for line in &new[hunk.new.clone()] {
                push_line(&mut out, b'+', line);
            }
            at = hunk.old.end;
        }
        for line in &old[at..block.old.end] {
            push_line(&mut out, b' ', line);
        }
    }
    Ok(out)
}

/// 补丁里的一块改动：`old`、`new` 是带上上下文以后的行区间，`hunks` 是块内的改动。
struct Block {
    old: Range<usize>,
    new: Range<usize>,
    hunks: Vec<LineHunk>,
}

/// 把各段改动前后补上上下文，重叠或相接的并成一块：同一段内容不能出现在两块里。
fn blocks(hunks: &[LineHunk], old_len: usize) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for hunk in hunks {
        let before = PATCH_CONTEXT.min(hunk.old.start);
        // 改动之后的公共行在两侧一样多，按旧侧的数量补即可。
        let after = PATCH_CONTEXT.min(old_len - hunk.old.end);
        let old = hunk.old.start - before..hunk.old.end + after;
        let new = hunk.new.start - before..hunk.new.end + after;
        match blocks.last_mut() {
            Some(last) if old.start <= last.old.end => {
                last.old.end = last.old.end.max(old.end);
                last.new.end = last.new.end.max(new.end);
                last.hunks.push(hunk.clone());
            }
            _ => blocks.push(Block {
                old,
                new,
                hunks: vec![hunk.clone()],
            }),
        }
    }
    blocks
}

/// 把补丁里的一行写进 `out`；这一行没有换行符时按补丁的写法标注出来。
fn push_line(out: &mut Vec<u8>, prefix: u8, line: &str) {
    out.push(prefix);
    out.extend_from_slice(line.as_bytes());
    if !line.ends_with('\n') {
        out.extend_from_slice(b"\n\\ No newline at end of file\n");
    }
}

/// 返回补丁头部里的文件名；含制表符、换行符、引号或反斜杠时按 Git 的写法加引号。
fn patch_name(prefix: &str, name: &[u8]) -> Vec<u8> {
    let quoted = name
        .iter()
        .any(|b| matches!(b, b'\t' | b'\n' | b'\r' | b'"' | b'\\'));
    let mut out = Vec::with_capacity(prefix.len() + name.len() + 2);
    if quoted {
        out.push(b'"');
    }
    out.extend_from_slice(prefix.as_bytes());
    for &byte in name {
        match byte {
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            _ => out.push(byte),
        }
    }
    if quoted {
        out.push(b'"');
    }
    out
}

/// 把补丁要比较的内容当成文本；不是 UTF-8 或含 NUL 时返回 `None`（二进制内容无法逐行映射）。
fn text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    (!text.contains('\0')).then_some(text)
}

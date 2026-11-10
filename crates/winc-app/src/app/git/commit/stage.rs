//! 暂存与取消暂存：先保存相关的文档，再在后台写暂存区，完成后刷新状态和差异。
//!
//! 写入分三种：整文件（`git add` / `git restore --staged`）、整个仓库，以及改动块和单行（把目标
//! 内容交给 [`Git::stage_partial`]）。内容在保存以后按实际读到的东西重新算，选择对不上时停下并
//! 提示重新选择，不会退化成整文件暂存。

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use winc_core::diff::{LineSet, Pick, hunk_key, line_hunks, lines, pick_changes};
use winc_editor::diff_view::LEFT;
use winc_editor::git::merge::file_has_conflicts;
use winc_i18n::t;

use super::*;
use crate::app::lifecycle::notice::NoticeAction;

/// 一次暂存请求要写进暂存区的内容。
enum Job {
    /// 整文件：`stage` 为真时暂存，否则取消暂存。
    Whole { stage: bool, paths: Vec<PathBuf> },
    /// 整个仓库。
    All { stage: bool },
    /// 部分内容：暂存区里 `path` 的内容改成 `content`。
    Partial {
        stage: bool,
        path: PathBuf,
        content: Vec<u8>,
    },
}

/// 后台交回来的暂存结果。
pub(super) struct Staged {
    /// 在哪个仓库里写的。
    pub root: PathBuf,
    /// 请求的序号（[`CommitState::stage_seq`]）：只清除发起它的那次请求的状态。
    pub seq: u64,
    /// 是暂存；`false` 是取消暂存。
    pub stage: bool,
    /// 这次请求涉及的文件数；整个仓库时为 0。
    pub files: usize,
    /// 结果；失败时是 Git 给出的原因。
    pub result: Result<(), String>,
}

/// 差异里的一个部分暂存选择：一块改动，或者其中的一行。
///
/// 选择按块的内容键记录（[`hunk_key`]），不看行号：保存、格式化或外部修改让行号变化以后，按键
/// 重新找到那一块。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StagePick {
    /// 改动块的内容键。
    pub key: u64,
    /// 块在旧内容里的起始行：同样的改动有几处时用它区分。
    pub old: usize,
    /// 块内要选中的行：（侧，相对块首的行号）；`None` 是整块。
    pub line: Option<(usize, usize)>,
}

/// 正在写暂存区的工作树（规范化后的根目录）。
///
/// 同一应用内多个项目可以指向同一个工作树，写操作在其中共用占用；不同工作树互不影响。与其他
/// 进程之间仍由 Git 自己的索引锁协调。
static BUSY: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

fn busy() -> MutexGuard<'static, Option<HashSet<PathBuf>>> {
    BUSY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 占用工作树 `root`，返回是否成功；已被占用（本轮或另一个项目）时返回 `false`。
pub(super) fn claim(root: &Path) -> bool {
    busy()
        .get_or_insert_with(HashSet::new)
        .insert(winc_editor::canonical_path(root))
}

/// 放开工作树 `root`。
pub(super) fn release(root: &Path) {
    if let Some(worktrees) = busy().as_mut() {
        worktrees.remove(&winc_editor::canonical_path(root));
    }
}

impl App {
    /// 暂存或取消暂存 `files`（同一个组）：先保存这些文件里的未保存修改，再在后台写暂存区。
    ///
    /// 冲突文件仍有冲突标记时停下并说明。失败时状态栏显示 Git 的原因，完成后重新读取真实状态。
    pub(in crate::app) fn stage_files(&mut self, files: Vec<CommitFile>) {
        let Some(group) = files.first().map(|f| f.group) else {
            return;
        };
        let stage = group.stages();
        let files: Vec<CommitFile> = files.into_iter().filter(|f| f.group == group).collect();
        // 改名的条目连同旧路径一起处理：只处理新路径会把改名拆成一处新增和一处未暂存的删除。
        let mut paths = Vec::with_capacity(files.len());
        if let Some(status) = self.git.status.as_ref() {
            for file in &files {
                match status.entries.iter().find(|e| e.path == file.path) {
                    Some(entry) => {
                        paths.push(entry.path.clone());
                        paths.extend(entry.from.clone());
                    }
                    None => paths.push(file.path.clone()),
                }
            }
        }
        if stage && !self.prepare_stage(&files) {
            return;
        }
        let count = files.len();
        self.start_stage(count, stage, Job::Whole { stage, paths });
    }

    /// 暂存或取消暂存整个仓库的改动，范围不受状态列表的数量上限影响。
    ///
    /// 暂存前同样先保存仓库里打开的文档（见 [`App::prepare_stage`]）；取消暂存不动工作区，不保存。
    pub(in crate::app) fn stage_all(&mut self, stage: bool) {
        if stage && !self.prepare_stage_all() {
            return;
        }
        self.start_stage(0, stage, Job::All { stage });
    }

    /// 暂存（或取消暂存）`file` 的差异里选中的一块改动或一行。
    ///
    /// 暂存前先保存文件，然后按保存后的实际内容重新找到选择：找不到（内容已经变了）时刷新差异并
    /// 提示重新选择。取消暂存只改暂存区，不保存也不改工作区。
    pub(in crate::app) fn stage_pick(&mut self, file: CommitFile, pick: StagePick) {
        let stage = file.group.stages();
        if stage && !self.prepare_stage(std::slice::from_ref(&file)) {
            return;
        }
        let Some(content) = self.pick_content(&file, pick) else {
            return;
        };
        self.start_stage(
            1,
            stage,
            Job::Partial {
                stage,
                path: file.path,
                content,
            },
        );
    }

    /// 暂存前的准备：写操作互斥、冲突标记、保存未保存的修改。返回是否继续。
    fn prepare_stage(&mut self, files: &[CommitFile]) -> bool {
        if self.commit.busy || self.commit.staging || self.branch.running.is_some() {
            self.flash(t!("git-stage-busy"));
            return false;
        }
        let conflicts: Vec<&Path> = files
            .iter()
            .filter(|f| f.group == CommitGroup::Conflicts)
            .map(|f| f.path.as_path())
            .collect();
        for path in &conflicts {
            // 先保存：编辑器里刚移除冲突标记、磁盘上还是旧内容时，按磁盘上的结果判断。
            if !self.save_documents(std::slice::from_ref(&path.to_path_buf())) {
                return false;
            }
        }
        let unresolved = conflicts
            .iter()
            .filter(|path| file_has_conflicts(path))
            .count();
        if unresolved > 0 {
            self.flash(t!("git-stage-unresolved", count = unresolved));
            return false;
        }
        self.save_documents(&files.iter().map(|f| f.path.clone()).collect::<Vec<_>>())
    }

    /// 「暂存全部」前的准备，含义同 [`App::prepare_stage`]：保存的文档取自编辑器，不遍历状态
    /// 列表里可能被截断的条目；仍有冲突标记的文件停下并说明，避免 `git add --all` 把标记一并
    /// 暂存成已解决。
    pub(in crate::app::git) fn prepare_stage_all(&mut self) -> bool {
        if self.commit.busy || self.commit.staging || self.branch.running.is_some() {
            self.flash(t!("git-stage-busy"));
            return false;
        }
        let Some(root) = self.git.root.clone() else {
            self.flash(t!("git-not-a-repository"));
            return false;
        };
        let paths: Vec<PathBuf> = (self.editor.documents.iter())
            .filter_map(|d| d.path.clone())
            .filter(|path| path.starts_with(&root))
            .collect();
        if !self.save_documents(&paths) {
            return false;
        }
        let unresolved = (self.git.status.iter().flat_map(|s| s.entries.iter()))
            .filter(|e| e.conflicted() && file_has_conflicts(&e.path))
            .count();
        if unresolved > 0 {
            self.flash(t!("git-stage-unresolved", count = unresolved));
            return false;
        }
        true
    }

    /// 保存 `paths` 里未保存的文档，返回是否全部保存完成。
    ///
    /// 保存失败、磁盘冲突，或者保存前的格式化还没完成（文件仍有未保存的修改）时停下并说明：这时
    /// 暂存区里的内容会和用户在编辑器里看到的对不上。已经保存的文件保持已保存。
    pub(super) fn save_documents(&mut self, paths: &[PathBuf]) -> bool {
        for path in paths {
            let canonical = winc_editor::canonical_path(path);
            let Some(id) = self
                .editor
                .documents
                .iter()
                .find(|d| d.path.as_deref() == Some(canonical.as_path()))
                .map(|d| d.id)
            else {
                continue;
            };
            let waiting = self.is_formatting(id) || self.format_save_pending();
            if !waiting && !self.editor.document(id).is_some_and(|d| d.is_dirty()) {
                continue;
            }
            if !self.write_document(id, None, true) {
                return false;
            }
            // 还在等格式化、或者写入在后台进行：这时磁盘上的内容还不是用户在编辑器里看到的内容。
            if self.editor.document(id).is_some_and(|d| d.is_dirty()) {
                let name = winc_editor::display_name(path).into_owned();
                self.flash(t!("git-stage-save-pending", name = name));
                return false;
            }
        }
        true
    }

    /// 按 `file` 现在的实际内容算出选中 `pick` 之后暂存区应有的内容；选择对不上时刷新并返回
    /// `None`。
    fn pick_content(&mut self, file: &CommitFile, pick: StagePick) -> Option<Vec<u8>> {
        let root = self.git.root.clone()?;
        let Some(git) = self.git.tool.get() else {
            self.flash(t!("git-not-a-repository"));
            return None;
        };
        let cancel = winc_editor::format::Cancel::default();
        let staged = file.group == CommitGroup::Staged;
        let name = winc_editor::display_name(&file.path).into_owned();
        // 符号链接的内容是链接目标，磁盘上读到的是目标文件：两侧对不上，只提供整文件操作。
        if !staged
            && std::fs::symlink_metadata(&file.path).is_ok_and(|m| m.file_type().is_symlink())
        {
            self.flash(t!("git-stage-partial-symlink", name = name));
            return None;
        }
        // 两侧的原始内容：暂存看 `HEAD` 和暂存区，取消暂存看 `HEAD` 和暂存区，更改看暂存区和磁盘。
        let read = |path: &Path| -> Result<Option<Vec<u8>>, String> {
            if staged {
                git.head_content(&root, path, &cancel)
            } else {
                git.index_content(&root, path, &cancel)
            }
        };
        let old = match read(&file.path) {
            Ok(bytes) => bytes.unwrap_or_default(),
            Err(e) => {
                self.notify(t!("git-stage-failed", error = e));
                return None;
            }
        };
        let new = if staged {
            match git.index_content(&root, &file.path, &cancel) {
                Ok(bytes) => bytes.unwrap_or_default(),
                Err(e) => {
                    self.notify(t!("git-stage-failed", error = e));
                    return None;
                }
            }
        } else {
            match std::fs::read(&file.path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    self.notify(t!(
                        "git-read-file-failed",
                        name = name,
                        error = e.to_string()
                    ));
                    return None;
                }
            }
        };
        // 部分暂存按 UTF-8 文本映射暂存区里的字节：带 BOM 或不是 UTF-8 时只提供整文件操作。
        let has_bom = |bytes: &[u8]| bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        if has_bom(&old) || has_bom(&new) {
            self.flash(t!("git-stage-partial-bom", name = name));
            return None;
        }
        let (Ok(old), Ok(new)) = (std::str::from_utf8(&old), std::str::from_utf8(&new)) else {
            self.flash(t!("git-stage-partial-not-utf8", name = name));
            return None;
        };
        let (a, b) = (lines(old), lines(new));
        let all: Vec<(u64, std::ops::Range<usize>, std::ops::Range<usize>)> =
            (line_hunks(old, new).into_iter())
                .map(|hunk| {
                    let key = hunk_key(
                        a[hunk.old.clone()].iter().copied(),
                        b[hunk.new.clone()].iter().copied(),
                    );
                    (key, hunk.old, hunk.new)
                })
                .collect();
        let found: Vec<usize> = (all.iter().enumerate())
            .filter(|(_, (key, _, _))| *key == pick.key)
            .map(|(i, _)| i)
            .collect();
        // 同样的改动只有一处时按内容认（行号会因保存或格式化而移动）；有几处时按块首行号认，
        // 对不上就停下，不再猜是哪一处。
        let target = match found[..] {
            [one] => Some(one),
            _ => (found.iter().copied()).find(|i| all[*i].1.start == pick.old),
        };
        let Some(target) = target else {
            self.flash(t!("git-stage-selection-stale"));
            self.git_refresh();
            return None;
        };
        // 方向：`staged` 是「已暂存的更改」那一侧，对它做的操作是取消暂存，其余是暂存。
        let picks: Vec<Pick> = (all.iter().enumerate())
            .map(|(i, (_, old, new))| {
                if i != target {
                    return if staged { Pick::Apply } else { Pick::Skip };
                }
                match pick.line {
                    // 整块：暂存时整块应用，取消暂存时整块留下。
                    None if staged => Pick::Skip,
                    None => Pick::Apply,
                    // 暂存一行：其他行都不应用。
                    Some((side, line)) if !staged => {
                        let mut set = line_set(old.len(), new.len());
                        toggle_line(&mut set, side, line);
                        Pick::Lines(set)
                    }
                    // 取消暂存一行：其他行都留下。
                    Some((side, line)) => {
                        let mut set = LineSet::default();
                        toggle_line(&mut set, side, line);
                        Pick::Lines(set)
                    }
                }
            })
            .collect();
        let content = pick_changes(old, new, |key, old| {
            (all.iter().position(|(k, o, _)| *k == key && *o == old))
                .map_or(if staged { Pick::Apply } else { Pick::Skip }, |i| {
                    picks[i].clone()
                })
        });
        Some(content.into_bytes())
    }

    /// 在后台写暂存区；`files` 是涉及的文件数（整个仓库时为 0）。
    fn start_stage(&mut self, files: usize, stage: bool, job: Job) {
        if self.commit.busy {
            self.flash(t!("git-stage-busy"));
            return;
        }
        let Some(root) = self.git.root.clone() else {
            self.flash(t!("git-not-a-repository"));
            return;
        };
        if self.commit.staging || self.branch.running.is_some() {
            self.flash(t!("git-stage-busy"));
            return;
        }
        if !claim(&root) {
            self.flash(t!("git-stage-busy"));
            return;
        }
        self.commit.staging = true;
        self.commit.stage_seq += 1;
        let seq = self.commit.stage_seq;
        let sender = self.commit.stage_results.0.clone();
        self.run_git_job(move |git, root| {
            let result = git.and_then(|git| match job {
                Job::Whole { stage: true, paths } => git.stage(root, &paths).map(|_| ()),
                Job::Whole {
                    stage: false,
                    paths,
                } => git.unstage(root, &paths),
                Job::All { stage: true } => git.stage_all(root),
                Job::All { stage: false } => git.unstage_all(root),
                Job::Partial {
                    stage: true,
                    path,
                    content,
                } => git.stage_partial(root, &path, &content),
                Job::Partial {
                    stage: false,
                    path,
                    content,
                } => git.unstage_partial(root, &path, &content),
            });
            // 暂存区写完就放开工作树：发起它的项目随后关闭时结果会被丢弃，占用不能留给同一进程里
            // 的其他项目。
            release(root);
            let _ = sender.send(Staged {
                root: root.to_path_buf(),
                seq,
                stage,
                files,
                result,
            });
        });
    }

    /// 收下后台交回来的暂存结果：成功与失败都重新读取真实状态，失败时显示 Git 的原因。
    pub(in crate::app) fn poll_stage(&mut self) {
        while let Ok(done) = self.commit.stage_results.1.try_recv() {
            if self.git.root.as_deref() != Some(done.root.as_path()) {
                // 等结果的时候换了仓库：只说一声结果，不动现在这个仓库的状态。
                let name = winc_editor::display_name(&done.root).into_owned();
                match done.result {
                    Ok(()) if done.stage => {
                        self.flash(t!("git-stage-done-in", name = name, count = done.files))
                    }
                    Ok(()) => {
                        self.flash(t!("git-unstage-done-in", name = name, count = done.files))
                    }
                    Err(error) => {
                        self.notify(t!("git-stage-failed-in", name = name, error = error))
                    }
                }
                continue;
            }
            if done.seq != self.commit.stage_seq {
                continue;
            }
            self.commit.staging = false;
            match done.result {
                Ok(()) if done.stage => self.flash(t!("git-stage-done", count = done.files)),
                Ok(()) => self.flash(t!("git-unstage-done", count = done.files)),
                Err(error) => self.notify_with(
                    t!("git-stage-failed", error = error),
                    NoticeAction::GitOutput,
                ),
            }
            self.git_refresh();
        }
    }
}

/// 全部行都不应用。
fn line_set(old: usize, new: usize) -> LineSet {
    LineSet {
        old: (0..old).collect(),
        new: (0..new).collect(),
    }
}

/// 把 `side` 侧第 `line` 行（相对改动开头）加入或移出 `set`。
fn toggle_line(set: &mut LineSet, side: usize, line: usize) {
    let lines = if side == LEFT {
        &mut set.old
    } else {
        &mut set.new
    };
    if !lines.remove(&line) {
        lines.insert(line);
    }
}

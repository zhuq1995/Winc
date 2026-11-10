//! 提交和回滚：提交暂存区里的内容，提交说明，撤销上一次提交。
//!
//! [`Git::rollback`] 把文件恢复成 `HEAD` 里的内容；[`Git::commit`] 提交真实暂存区，未暂存的改动留在
//! 工作区。暂存与取消暂存见 [`Git::stage`]、[`Git::unstage`] 和它们的部分暂存版本。

use super::*;

impl Git {
    /// 回滚已跟踪的 `paths`：暂存区和工作区都恢复成 `HEAD` 里的内容（`git restore`）。
    ///
    /// `added` 是 `HEAD` 里没有、新加入版本管理的文件，只从暂存区移除，工作区里的文件由调用方处理。
    ///
    /// # Errors
    ///
    /// `git` 报错时返回它的说明。
    pub fn rollback(
        &self,
        root: &Path,
        paths: &[PathBuf],
        added: &[PathBuf],
    ) -> Result<(), String> {
        self.require_trust(root)?;
        let cancel = Cancel::default();
        if !added.is_empty() {
            let mut command = self.command(root);
            command
                .args(["rm", "--cached", "--quiet", "--"])
                .args(relative_all(root, added));
            self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
        }
        if !paths.is_empty() {
            let mut command = self.command(root);
            command
                .args(["restore", "--source=HEAD", "--staged", "--worktree", "--"])
                .args(relative_all(root, paths));
            self.run(&mut command, STATUS_TIMEOUT, &cancel)?;
        }
        Ok(())
    }

    /// 提交暂存区里的内容（`git commit`），返回新提交号的前 7 位。
    ///
    /// 提交的内容就是真实暂存区：未暂存的改动留在工作区，暂存区里其他文件一并进入这次提交。
    /// `amend` 为 `true` 时修改上一次提交，暂存区没有变化时只改提交信息。合并、拣选、还原进行中
    /// 同样提交整个暂存区，这样得到的才是一个合并提交。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、`git` 报错（hooks 拒绝、没有可提交的内容、身份没配置……）时返回它的说明。
    pub fn commit(&self, root: &Path, message: &str, amend: bool) -> Result<String, String> {
        self.require_trust(root)?;
        let mut command = self.command(root);
        // `whitespace` 只去掉首尾空行和行尾空白；`strip` 还会删掉 `#` 开头的行，
        // 「#42 修复登录」这样的信息会被整段删除。
        command.args(["commit", "--file=-", "--cleanup=whitespace"]);
        if amend {
            command.arg("--amend");
        }
        self.run_commit(root, &mut command, message)?;
        self.short_head(root)
    }

    /// 运行 `git commit`，提交信息由标准输入传入。
    fn run_commit(&self, root: &Path, command: &mut Command, message: &str) -> Result<(), String> {
        command.stderr(Stdio::piped());
        match process::run(
            command,
            message.as_bytes().to_vec(),
            COMMIT_TIMEOUT,
            &Cancel::default(),
        ) {
            Ok(output) => {
                // hooks 的输出可能在标准输出里。
                let mut text = output.stderr;
                text.extend_from_slice(&output.stdout);
                output::record(root, &["commit"], output.status.code(), &text);
                if output.status.success() {
                    Ok(())
                } else {
                    Err(failure(&text))
                }
            }
            Err(e) => {
                let reason = describe(&e);
                output::record(root, &["commit"], None, reason.as_bytes());
                Err(reason)
            }
        }
    }

    /// 返回 `HEAD` 的提交号（前 7 位）。
    fn short_head(&self, root: &Path) -> Result<String, String> {
        let mut command = self.command(root);
        command.args(["rev-parse", "--short", "HEAD"]);
        let oid = self.run(&mut command, SHOW_TIMEOUT, &Cancel::default())?;
        Ok(String::from_utf8_lossy(&oid).trim().to_owned())
    }

    /// 返回 `HEAD` 指向的提交；还没有提交过、`git` 运行失败时为 `None`。
    pub fn last_commit(&self, root: &Path) -> Option<LastCommit> {
        let mut command = self.command(root);
        command.args([
            "log",
            "-1",
            "--date=format-local:%Y-%m-%d %H:%M",
            "--format=%H%x00%ct%x00%ad%x00%B",
        ]);
        let stdout = self
            .run(&mut command, SHOW_TIMEOUT, &Cancel::default())
            .ok()?;
        let text = String::from_utf8_lossy(&stdout);
        let mut fields = text.splitn(4, '\0');
        let oid = fields.next()?.trim().to_owned();
        let time = fields.next()?.trim().parse().ok()?;
        let local = fields.next()?.to_owned();
        let message = fields.next()?.trim_end().to_owned();
        let subject = message.lines().next().unwrap_or_default().to_owned();
        (!oid.is_empty()).then_some(LastCommit {
            oid,
            subject,
            message,
            time,
            local,
        })
    }

    /// 撤销 `HEAD` 指向的提交 `oid`：分支退回上一个提交，这次提交的改动留在暂存区，工作区不动
    /// （同 `git reset --soft HEAD~1`）；`oid` 是第一个提交时删掉分支的引用，改动同样留在暂存区。
    ///
    /// 只在分支仍指向 `oid` 时改动（`update-ref` 带旧值），之后又提交过、换了分支时不撤销。
    ///
    /// # Errors
    ///
    /// 仓库不受信任、`HEAD` 已经不是 `oid`、`git` 报错时返回说明。
    pub fn undo_commit(&self, root: &Path, oid: &str) -> Result<(), String> {
        self.require_trust(root)?;
        let cancel = Cancel::default();
        let mut command = self.command(root);
        command.args(["rev-parse", "--verify", "--quiet", &format!("{oid}^")]);
        let parent = self.run(&mut command, SHOW_TIMEOUT, &cancel).ok();
        let parent = parent.map(|p| String::from_utf8_lossy(&p).trim().to_owned());
        let mut command = self.command(root);
        command.args(["update-ref", "-m", "reset: undo commit"]);
        match parent.filter(|p| !p.is_empty()) {
            Some(parent) => command.args(["HEAD", &parent, oid]),
            None => command.args(["-d", "HEAD", oid]),
        };
        self.run(&mut command, SHOW_TIMEOUT, &cancel)?;
        Ok(())
    }

    /// 返回上一次提交的完整提交信息；还没有提交过、`git` 运行失败时为空字符串。
    pub fn last_message(&self, root: &Path) -> String {
        let mut command = self.command(root);
        command.args(["log", "-1", "--format=%B"]);
        match self.run(&mut command, SHOW_TIMEOUT, &Cancel::default()) {
            Ok(stdout) => String::from_utf8_lossy(&stdout).trim_end().to_owned(),
            Err(_) => String::new(),
        }
    }
}

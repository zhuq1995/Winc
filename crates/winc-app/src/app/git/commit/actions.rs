//! 提交和回滚：先保存要提交的文件、请用户确认回滚，在后台运行 git 并收下结果。

use winc_editor::git::Synced;
use winc_i18n::t;

use super::stage::{claim, release};
use super::*;
use crate::app::git::branch::{Then, keep_changes};
use crate::app::lifecycle::notice::NoticeAction;

/// 分支行右端同步控件的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncState {
    /// 仓库没有配置远程。
    NoRemote,
    /// 这时不同步：不在分支上、有进行中的操作，或者在修改上一次提交。
    Hidden,
    /// 和上游一致。
    InSync,
    /// 分支还没有上游：发布到远程。
    Publish,
    /// 领先上游这么多个提交：推送。
    Push(u32),
    /// 落后上游这么多个提交：拉取。
    Pull(u32),
    /// 两边都有新提交：同步。
    Both {
        /// 领先的提交数。
        ahead: u32,
        /// 落后的提交数。
        behind: u32,
    },
}

/// 需要确认的推送（见 [`App::ask_push`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PushIntent {
    /// 把还没有上游的当前分支发布到这个远程，并设为上游。
    Publish(String),
    /// 把当前分支推送到上游。
    Push,
    /// 同步：拉取上游的新提交，再推送本地的提交。
    Sync,
}

/// 提交成功后按钮显示「✓ 已提交」的时长（经验值）。
///
/// 短于状态栏提示的显示时长，足以让用户确认提交已完成。
pub(in crate::app) const COMMITTED_SHOWN: Duration = Duration::from_millis(1600);

impl App {
    /// 分支和上游的关系决定的同步控件状态。
    pub(crate) fn git_sync_state(&self) -> SyncState {
        let Some(status) = self.git.status.as_ref() else {
            return SyncState::Hidden;
        };
        if self.git.remotes.is_empty() {
            return SyncState::NoRemote;
        }
        let on_branch = matches!(status.head, Some(Head::Branch(_)));
        if !on_branch || status.operation.is_some() || self.commit.amend {
            return SyncState::Hidden;
        }
        // 分支列表还没读到时不知道有没有上游。
        if self.git.branches.is_empty() {
            return SyncState::Hidden;
        }
        if !self.has_upstream() {
            return SyncState::Publish;
        }
        match (status.ahead, status.behind) {
            (0, 0) => SyncState::InSync,
            (ahead, 0) => SyncState::Push(ahead),
            (0, behind) => SyncState::Pull(behind),
            (ahead, behind) => SyncState::Both { ahead, behind },
        }
    }

    /// 点击同步控件：有差异时同步，没有上游时发布，其余时候获取远程更新。
    ///
    /// 会推送的发布和同步先确认（[`App::ask_push`]）；只拉取时不确认。
    pub(in crate::app) fn git_sync(&mut self) {
        match self.git_sync_state() {
            SyncState::Publish => self.run_branch_command(MenuCommand::GitPush),
            SyncState::Push(_) | SyncState::Both { .. } => self.ask_push(PushIntent::Sync),
            SyncState::Pull(_) => self.run_git_sync(false),
            SyncState::NoRemote | SyncState::Hidden | SyncState::InSync => {
                self.run_branch_command(MenuCommand::GitFetch);
            }
        }
    }

    /// 在后台同步（[`winc_editor::git::Git::sync`]）；需要强制推送而 `force` 为假时转为询问。
    pub(in crate::app) fn run_git_sync(&mut self, force: bool) {
        self.git_operate_then(t!("git-op-sync").into(), move |git, root| {
            let (synced, then) =
                keep_changes(git, root, t!("git-op-sync"), |git| git.sync(root, force))?;
            Ok(match synced {
                Synced::Done(summary) => (summary, then),
                Synced::NeedsForce(count) => (String::new(), Then::ConfirmForcePush(count)),
            })
        });
    }

    /// 发布、推送或同步当前分支前用提示框确认，写明推到哪个远程仓库；配置项 `git.confirmPush`
    /// 为 `false`（提示框上勾选过「不再询问」）时不询问，直接执行。
    pub(in crate::app) fn ask_push(&mut self, intent: PushIntent) {
        if self.config.git_confirm_push == Some(false) {
            self.run_push(intent);
            return;
        }
        let (message, detail, button) = self.push_prompt(&intent);
        let buttons = vec![
            AlertButton::new(&button),
            AlertButton::cancel(t!("common-cancel")),
        ];
        let asked = Some((t!("common-dont-ask"), false));
        self.show_alert(&message, &detail, buttons, asked, move |choice, never| {
            (choice == 0).then(|| UserEvent::GitPushConfirmed(intent.clone(), never))
        });
    }

    /// 提示框中确认了 `intent`：勾选了「不再询问」（`never`）时写入配置，然后执行。
    pub(in crate::app) fn push_confirmed(&mut self, intent: PushIntent, never: bool) {
        if never {
            self.config.git_confirm_push = Some(false);
            self.save_config();
        }
        self.run_push(intent);
    }

    /// 确认 `intent` 的提示框内容：（标题，说明，确认按钮的文字）。
    pub(in crate::app) fn push_prompt(&self, intent: &PushIntent) -> (String, String, String) {
        let current = self.git.branches.iter().find(|b| b.current && !b.remote);
        let branch = current.map_or("HEAD", |b| b.name.as_str());
        let upstream = current.and_then(|b| b.upstream.as_deref()).unwrap_or("");
        let (ahead, behind) = (self.git.status.as_ref()).map_or((0, 0), |s| (s.ahead, s.behind));
        // 上游写作「远程名/分支名」，远程名本身可以含 `/`，按已知的远程名匹配最长的一个。
        let upstream_remote = (self.git.remotes.iter())
            .filter(|r| upstream.starts_with(&format!("{r}/")))
            .max_by_key(|r| r.len())
            .map(String::as_str);
        // 推送的目标写远程的地址；取不到地址时写远程名。
        let target = |remote: &str| {
            (self.git.remote_list.iter())
                .find(|r| r.name == remote)
                .map_or_else(|| remote.to_owned(), |r| r.url.clone())
        };
        let upstream_target = target(upstream_remote.unwrap_or(upstream));
        let (message, detail, button) = match intent {
            PushIntent::Publish(remote) => (
                t!("git-push-confirm-publish", remote = remote.as_str()),
                t!(
                    "git-push-confirm-publish-detail",
                    branch = branch,
                    target = target(remote)
                ),
                t!("git-push-confirm-publish-button"),
            ),
            PushIntent::Push => (
                t!("git-push-confirm-push", upstream = upstream),
                t!(
                    "git-push-confirm-push-detail",
                    count = ahead,
                    target = upstream_target
                ),
                t!("git-push-confirm-push-button"),
            ),
            PushIntent::Sync => (
                t!("git-push-confirm-sync", upstream = upstream),
                t!(
                    "git-push-confirm-sync-detail",
                    ahead = ahead,
                    behind = behind,
                    target = upstream_target
                ),
                t!("git-push-confirm-sync-button"),
            ),
        };
        (message.to_owned(), detail.to_owned(), button.to_owned())
    }

    /// 执行确认过的 `intent`：发布推到确认时写明的远程。
    pub(in crate::app) fn run_push(&mut self, intent: PushIntent) {
        match intent {
            PushIntent::Publish(remote) => {
                self.git_operate(t!("git-op-push").into(), move |git, root| {
                    git.publish(root, &remote)
                });
            }
            PushIntent::Push => {
                self.git_operate(t!("git-op-push").into(), |git, root| git.push(root, true))
            }
            PushIntent::Sync => self.run_git_sync(false),
        }
    }

    /// 同步需要强制推送：用提示框询问，确认后才替换上游的 `count` 个提交。
    pub(in crate::app) fn confirm_force_push(&mut self, count: usize) {
        let branch = (self.git.branches.iter())
            .find(|b| b.current)
            .map_or("HEAD", |b| b.name.as_str());
        let message = t!("git-force-push-confirm", branch = branch);
        let detail = t!("git-force-push-confirm-detail", count = count);
        let button = t!("git-force-push-confirm-button");
        self.show_confirm(&message, &detail, button, |ok| {
            ok.then_some(UserEvent::GitForcePush)
        });
    }

    /// 提交并推送：可以提交时用提示框询问，写明提交多少文件、推到哪个分支；不能提交时由提交的入口
    /// 说明原因。暂存区为空时只问「暂存全部并提交」，那一个提示框也写明提交后会推送。
    pub(in crate::app) fn confirm_commit_and_push(&mut self) {
        let staged = (self.git.status.as_ref()).is_some_and(|status| {
            (status.entries.iter()).any(|e| !e.conflicted() && e.staged.is_some())
        });
        let ready = !self.commit.busy
            && !self.commit.staging
            && (staged || self.commit.amend)
            && (self.git.status.as_ref())
                .is_some_and(|status| self.commit.blocker(status).is_none());
        if !ready {
            self.git_commit_and_push();
            return;
        }
        let branch = self
            .git
            .branches
            .iter()
            .find(|b| b.current)
            .map(|b| b.name.clone());
        let detail = match (self.commit_scope(), branch) {
            (Some(count), Some(branch)) => t!(
                "git-commit-push-confirm-scope",
                count = count,
                branch = branch.as_str()
            ),
            _ => t!("git-commit-push-confirm-detail").to_owned(),
        };
        let buttons = vec![
            AlertButton::new(t!("git-commit-push-confirm-button")),
            AlertButton::cancel(t!("common-cancel")),
        ];
        let message = t!("git-commit-push-confirm");
        self.show_alert(message, &detail, buttons, None, |choice, _| {
            (choice == 0).then_some(UserEvent::GitCommitAndPush)
        });
    }

    /// 提交暂存区里的内容。
    ///
    /// 没写提交信息时在状态栏说明，不提交。暂存区为空时有可提交的改动就确认是否暂存全部并提交；
    /// 没有可提交的改动时同样说明。变基停下来时继续变基。
    pub(in crate::app) fn git_commit(&mut self) {
        if self.commit.busy || self.commit.staging {
            return;
        }
        let message = self.commit.message.text().trim().to_owned();
        let rebasing = self
            .git
            .status
            .as_ref()
            .is_some_and(|s| s.operation == Some(Operation::Rebase));
        if message.is_empty() && !rebasing {
            self.flash(t!("git-commit-message-required"));
            self.focus = Focus::CommitMessage;
            return;
        }
        // 保存冲突文件需要写状态，这里拿副本，避免长时借用。
        let Some(status) = self.git.status.clone() else {
            return;
        };
        // 冲突优先：仍有冲突标记的文件先处理，否则 `git add` 会把标记和两侧内容一并放进提交。
        // 先保存：编辑器里刚移除标记、磁盘上还是旧内容时，按磁盘上的结果判断。
        let conflicts: Vec<PathBuf> = (status.entries.iter())
            .filter(|e| e.conflicted())
            .map(|e| e.path.clone())
            .collect();
        if !self.save_documents(&conflicts) {
            return;
        }
        let unresolved = conflicts
            .iter()
            .filter(|path| winc_editor::git::merge::file_has_conflicts(path))
            .count();
        if unresolved > 0 {
            self.flash(t!("git-commit-unresolved-conflicts", count = unresolved));
            return;
        }
        // 变基停下来时继续变基：已解决的文件由显式暂存入口处理，`git` 用原来的提交信息。
        if rebasing {
            self.git_operate(t!("git-op-continue-rebase").to_owned(), move |git, root| {
                git.continue_operation(root, Operation::Rebase)
            });
            return;
        }
        if status.entries.is_empty() && !self.commit.amend {
            self.flash(t!("git-commit-nothing-to-commit"));
            return;
        }
        let staged = (status.entries.iter())
            .filter(|e| !e.conflicted() && e.staged.is_some())
            .count();
        if staged == 0 && !self.commit.amend {
            // 暂存区为空：由用户确认暂存全部并提交。列表截断时「全部」仍是当前仓库。推送意图
            // 先收起来，确认以后再恢复：取消确认不留着它。
            let push = std::mem::take(&mut self.commit.push_after);
            self.confirm_stage_all_commit(push);
            return;
        }
        self.commit_staged(message);
    }

    /// 确认「暂存全部并提交」：文案写明范围是当前仓库，包含未跟踪且未被忽略的文件。`push` 为真时
    /// 提交完成后继续推送。
    fn confirm_stage_all_commit(&mut self, push: bool) {
        let Some(root) = self.git.root.clone() else {
            self.flash(t!("git-not-a-repository"));
            return;
        };
        let name = winc_editor::display_name(&root).into_owned();
        let message = t!("git-commit-stage-all-confirm");
        let mut detail = t!("git-commit-stage-all-detail", name = name.as_str()).to_owned();
        if push {
            detail.push('\n');
            detail.push_str(t!("git-commit-stage-all-push"));
        }
        let button = t!("git-commit-stage-all-button");
        self.show_confirm(message, &detail, button, move |ok| {
            ok.then_some(UserEvent::GitStageAllCommit(push))
        });
    }

    /// 确认之后：保存→暂存全部→提交，串行执行；哪一步失败就停在哪一步。
    ///
    /// 暂存成功、提交失败时保留真实的暂存区和提交信息，重试不会再次加入工作区后续的变化。
    pub(in crate::app) fn git_stage_all_and_commit(&mut self) {
        if self.commit.busy || self.commit.staging {
            return;
        }
        let message = self.commit.message.text().trim().to_owned();
        if message.is_empty() {
            self.flash(t!("git-commit-message-required"));
            self.focus = Focus::CommitMessage;
            return;
        }
        if !self.prepare_stage_all() {
            return;
        }
        self.commit_staged_after_staging_all(message);
    }

    /// 提交真实暂存区（`git commit`，修改上次提交时加 `--amend`）。
    fn commit_staged(&mut self, message: String) {
        let amend = self.commit.amend;
        let sender = self.commit.results.0.clone();
        self.commit.busy = true;
        self.run_git_job(move |git, root| {
            let result = git.and_then(|git| git.commit(root, &message, amend));
            let _ = sender.send(Outcome::Committed(result, message, root.to_path_buf()));
        });
    }

    /// 先暂存整个仓库再提交（空暂存区确认之后）。
    fn commit_staged_after_staging_all(&mut self, message: String) {
        let amend = self.commit.amend;
        let sender = self.commit.results.0.clone();
        let Some(root) = self.git.root.clone() else {
            return;
        };
        // 这一步先写暂存区：与其他项目写同一工作树时同样占用工作树。
        if !claim(&root) {
            self.flash(t!("git-stage-busy"));
            return;
        }
        self.commit.busy = true;
        self.run_git_job(move |git, root| {
            let result = git.and_then(|git| {
                git.stage_all(root)?;
                git.commit(root, &message, amend)
            });
            release(root);
            let _ = sender.send(Outcome::Committed(result, message, root.to_path_buf()));
        });
    }

    /// 确认后中止进行中的合并、变基等操作。
    ///
    /// 中止会丢弃已部分解决的冲突，因此须先确认。
    pub(super) fn confirm_abort(&mut self) {
        let Some(operation) = self.git.status.as_ref().and_then(|s| s.operation) else {
            return;
        };
        let label = operation.label();
        let message = t!("git-abort-confirm", operation = label);
        if self.window.handle.is_some() {
            let detail = t!("git-abort-confirm-detail");
            let button = t!("git-abort-confirm-button", operation = label);
            self.show_confirm(&message, detail, &button, move |ok| {
                ok.then_some(UserEvent::GitAbort)
            });
            return;
        }
        // 没有窗口（测试中）时直接中止。
        self.run_branch_command(crate::app::input::context_menu::MenuCommand::GitAbort);
    }

    /// 回滚前先确认（系统确认框）；确认后回滚这一组里的文件。
    ///
    /// 选区在这一组里时只回滚选中的，否则回滚整组；取消暂存是另一个入口（−），不影响工作区。
    pub(super) fn ask_rollback(&mut self, group: CommitGroup) {
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        let selected = self.commit_selection();
        let chosen = |path: &Path| {
            selected.is_empty() || selected.iter().any(|f| f.group == group && f.path == path)
        };
        let paths: Vec<PathBuf> = (status.entries.iter())
            .filter(|e| group.holds(e) && chosen(&e.path))
            .map(|e| e.path.clone())
            .collect();
        if paths.is_empty() {
            self.flash(t!("git-rollback-nothing-checked"));
            return;
        }
        self.confirm_rollback(paths);
    }

    /// 先请用户确认，确认后回滚 `paths`。
    pub(super) fn confirm_rollback(&mut self, paths: Vec<PathBuf>) {
        let message = match paths.as_slice() {
            [one] => {
                let name = winc_editor::display_name(one);
                t!("git-rollback-confirm-one", name = name)
            }
            many => t!("git-rollback-confirm-many", count = many.len()),
        };
        if self.window.handle.is_some() {
            let detail = t!("git-rollback-confirm-detail");
            let button = t!("git-rollback-confirm-button");
            self.show_confirm(&message, detail, button, move |ok| {
                ok.then(|| UserEvent::GitRollback(paths.clone()))
            });
            return;
        }
        // 没有窗口（测试中）时直接回滚。
        self.git_rollback(paths);
    }

    /// 回滚 `paths`：已跟踪的恢复成 `HEAD` 里的内容，新加入版本管理的从暂存区移除并移到废纸篓，
    /// 未跟踪的移到废纸篓。
    ///
    /// 打开着的文档由目录监视仍然重新读取。
    pub(in crate::app) fn git_rollback(&mut self, paths: Vec<PathBuf>) {
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        let mut restore = Vec::new();
        let mut added = Vec::new();
        let mut trash = Vec::new();
        for entry in status.entries.iter().filter(|e| paths.contains(&e.path)) {
            if entry.untracked() {
                trash.push(entry.path.clone());
            } else if entry.staged == Some(winc_editor::git::Change::Added) {
                added.push(entry.path.clone());
                trash.push(entry.path.clone());
            } else {
                restore.push(entry.path.clone());
                restore.extend(entry.from.clone());
            }
        }
        let count = paths.len();
        // 移到废纸篓在界面线程上做（和文件树的删除同一个入口），git 在后台。
        for path in &trash {
            if let Err(e) = (self.trash)(path) {
                let (name, error) = (winc_editor::display_name(path), e.to_string());
                self.notify(t!("git-rollback-file-failed", name = name, error = error));
            }
        }
        if restore.is_empty() && added.is_empty() {
            self.flash(t!("git-rollback-done", count = count));
            self.git_refresh();
            return;
        }
        let sender = self.commit.results.0.clone();
        self.commit.busy = true;
        self.run_git_job(move |git, root| {
            let result = git
                .and_then(|git| git.rollback(root, &restore, &added))
                .map(|()| count);
            let _ = sender.send(Outcome::RolledBack(result, root.to_path_buf()));
        });
    }

    /// 收下后台交回来的暂存、提交、回滚结果和上一次提交的信息。
    pub(in crate::app) fn poll_commit(&mut self) {
        // 暂存结果和提交结果都走 [`UserEvent::GitCommit`]，暂存先收。
        self.poll_stage();
        while let Ok(outcome) = self.commit.results.1.try_recv() {
            match outcome {
                Outcome::StashFiles(oid, name, files) => self.stash_files_arrived(oid, name, files),
                Outcome::StashDiff(title, view) => self.stash_diff_arrived(title, view),
                Outcome::LastMessage(message) => {
                    // 等结果的时候用户已经关闭了「修改上一次提交」，就不填。
                    if self.commit.amend {
                        self.commit.message.set_text(&message);
                        self.commit.message.set_caret(message.len(), false);
                    }
                }
                Outcome::Committed(result, message, root) => {
                    // 提交的时候已经换到了别的仓库：只说一声结果，不推送（会推到现在这个仓库）、
                    // 也不动现在这个仓库的提交视图（换的时候已经清过，见 `reset_commit_view`）。
                    if self.git.root.as_deref() != Some(root.as_path()) {
                        let name = winc_editor::display_name(&root);
                        match result {
                            Ok(oid) => self.flash(t!("git-commit-done-in", name = name, oid = oid)),
                            Err(e) => self.notify_with(
                                t!("git-commit-failed-in", name = name, error = e),
                                NoticeAction::GitOutput,
                            ),
                        }
                        continue;
                    }
                    self.commit.busy = false;
                    match result {
                        Ok(oid) => {
                            // 提交期间用户又改了信息，就留着改过的，不清空。
                            if self.commit.message.text().trim() == message {
                                self.commit.message.remember();
                                self.commit.message.set_text("");
                            }
                            let subject = message.lines().next().unwrap_or_default();
                            // 「提交并推送」已经确认过，推送不再询问。
                            if std::mem::take(&mut self.commit.push_after)
                                && let Some(intent) = self.push_intent()
                            {
                                self.run_push(intent);
                            }
                            self.commit.amend = false;
                            self.commit.draft = None;
                            self.flash(t!("git-commit-done", oid = &oid, subject = subject));
                            let short: String = oid.chars().take(7).collect();
                            self.commit.committed = Some((short, Instant::now() + COMMITTED_SHOWN));
                        }
                        Err(e) => {
                            self.commit.push_after = false;
                            self.notify_with(
                                t!("git-commit-failed", error = e),
                                NoticeAction::GitOutput,
                            );
                        }
                    }
                    self.git_refresh();
                }
                Outcome::Last(last, root) => {
                    let oid = self.git.status.as_ref().and_then(|s| s.oid.as_ref());
                    if self.git.root.as_deref() == Some(root.as_path())
                        && last.as_ref().map(|l| &l.oid) == oid
                    {
                        self.commit.last = last;
                    }
                }
                Outcome::Undone(result, last, root) => {
                    let name = winc_editor::display_name(&root);
                    let here = self.git.root.as_deref() == Some(root.as_path());
                    let tag: String = last.oid.chars().take(7).collect();
                    match result {
                        Ok(()) if here => {
                            if self.commit.message.text().trim().is_empty() {
                                self.commit.message.set_text(&last.message);
                                let end = last.message.len();
                                self.commit.message.set_caret(end, false);
                            }
                            self.flash(t!("git-undo-commit-done", oid = tag));
                        }
                        Ok(()) => self.flash(t!("git-undo-commit-done-in", name = name, oid = tag)),
                        Err(e) => {
                            self.commit.last_for = None;
                            self.notify(t!("git-undo-commit-failed", error = e));
                        }
                    }
                    if here {
                        self.git_refresh();
                    }
                }
                Outcome::RolledBack(result, root) => {
                    if self.git.root.as_deref() != Some(root.as_path()) {
                        let name = winc_editor::display_name(&root);
                        match result {
                            Ok(count) => {
                                self.flash(t!("git-rollback-done-in", name = name, count = count))
                            }
                            Err(e) => {
                                self.notify(t!("git-rollback-failed-in", name = name, error = e))
                            }
                        }
                        continue;
                    }
                    self.commit.busy = false;
                    match result {
                        Ok(count) => self.flash(t!("git-rollback-done", count = count)),
                        Err(e) => self.notify(t!("git-rollback-failed", error = e)),
                    }
                    // 重新读打开着的文档，顺带刷新仓库状态。
                    self.check_disk();
                }
            }
        }
        self.request_redraw();
    }

    /// 换了仓库：提交视图回到初始的样子（写了一半的信息、「修改上一次提交」、选区、正在提交都是
    /// 旧仓库的）；列表的显示方式（平铺、收起的分组）保留，旧仓库尚未交回的结果仍然收下。
    pub(in crate::app::git) fn reset_commit_view(&mut self) {
        let old = std::mem::take(&mut self.commit);
        self.commit.results = old.results;
        self.commit.stage_results = old.stage_results;
        self.commit.flat = old.flat;
        self.commit.collapsed = old.collapsed;
        self.commit.drafts = old.drafts;
    }

    /// 在后台对打开的仓库做一件提交视图的事（提交、回滚、暂存），做完以
    /// [`UserEvent::GitCommit`] 收结果（[`App::spawn_git`]）；找不到 git 时 `work` 收到原因。
    pub(super) fn run_git_job(
        &mut self,
        work: impl FnOnce(Result<&Git, String>, &Path) + Send + 'static,
    ) {
        let Some(root) = self.git.root.clone() else {
            return;
        };
        self.spawn_git(UserEvent::GitCommit, move |tool| {
            work(tool.as_ref().map_err(unusable), &root);
        });
    }
}

#[cfg(test)]
mod tests;

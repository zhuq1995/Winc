//! 暂存与取消暂存：勾选框读写暂存区。
//!
//! 界面先按操作的结果更新，命令在后台按顺序逐个执行；全部写完后重新读取状态。

use winc_core::diff::{LineHunk, StageLine, line_hunks, stage, unstage};
use winc_editor::format::Cancel;
use winc_editor::git::Change;

use super::*;
use crate::app::input::context_menu::MenuEntry;

/// 写入暂存区的一步操作。
pub(super) enum StageJob {
    /// 把这些文件的当前状态加入暂存区。
    Add(Vec<PathBuf>),
    /// 确认空索引提交后暂存整个仓库。
    AddAll,
    /// 把这些文件移出暂存区。
    Remove(Vec<PathBuf>),
    /// 暂存（`on` 为真）或取消暂存文件 `path` 的一段改动 `hunk`，或者其中的一行 `line`。
    Part {
        /// 文件的路径。
        path: PathBuf,
        /// 上次提交到工作区的一段改动。
        hunk: LineHunk,
        /// 只操作其中这一行；整段时为 `None`。
        line: Option<StageLine>,
        /// 暂存还是取消暂存。
        on: bool,
    },
}

impl App {
    /// 暂存（`on` 为真）或取消暂存 `paths` 中的文件：勾选框立即变化，命令在后台执行。
    ///
    /// 已经处于该状态的文件跳过；仍有冲突标记的文件不暂存，否则 `git` 将其视为已解决。
    pub(in crate::app) fn stage_paths(&mut self, paths: &[PathBuf], on: bool) {
        if self.git.commit.busy || self.git.branch.running.is_some() {
            return;
        }
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        let mut touched = Vec::new();
        let mut updates = Vec::new();
        let mut unresolved = 0;
        for (i, entry) in status
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| paths.contains(&e.path))
        {
            let dirty = on && self.stage_buffer(&entry.path);
            let mut next = entry.clone();
            if dirty && !entry.conflicted() {
                next.unstaged.get_or_insert(Change::Modified);
            }
            if on
                && next.unstaged.is_some()
                && entry.conflicted()
                && status.unresolved.contains(&entry.path)
            {
                unresolved += 1;
                continue;
            }
            let before = touched.len();
            flip_staged(&mut next, on, &mut touched);
            if touched.len() != before {
                updates.push((i, next));
            }
        }
        if on {
            for path in paths {
                if !status.entries.iter().any(|e| e.path == *path) && self.stage_buffer(path) {
                    touched.push(path.clone());
                }
            }
        }
        if unresolved > 0 {
            self.flash(t!("git-commit-unresolved-conflicts", count = unresolved));
        }
        if touched.is_empty() || on && !self.save_dirty_documents(&touched) {
            return;
        }
        if let Some(status) = self.git.status.as_mut() {
            for (i, next) in updates {
                if let Some(entry) = status.entries.get_mut(i).filter(|e| e.path == next.path) {
                    *entry = next;
                }
            }
        }
        self.git.changes.index_changed(touched.iter().cloned());
        let job = if on {
            StageJob::Add(touched)
        } else {
            StageJob::Remove(touched)
        };
        self.push_staging(job);
    }

    /// 暂存（`on` 为真）或取消暂存文件 `entry` 的一段改动 `hunk`，或者其中的一行 `line`。
    ///
    /// `texts` 是上次提交、暂存区、工作区三份文本，界面据此先行更新；后台按磁盘上的内容重新计算。
    /// 与暂存区比较的差异把暂存区放在上次提交一侧（两份都是暂存区），与上次提交比较的差异把
    /// 暂存区放在工作区一侧：只用到当前方向需要的两份。
    pub(in crate::app) fn stage_part(
        &mut self,
        entry: &Entry,
        texts: [&str; 3],
        (hunk, line): (LineHunk, Option<StageLine>),
        on: bool,
    ) {
        if self.git.commit.busy || self.git.branch.running.is_some() {
            return;
        }
        let [head, index, work] = texts;
        let next = if on {
            stage(head, index, work, &hunk, line)
        } else {
            unstage(head, index, work, &hunk, line)
        };
        if next == index {
            return;
        }
        let Some(status) = self.git.status.as_mut() else {
            return;
        };
        let Some(shown) = status.entries.iter_mut().find(|e| e.path == entry.path) else {
            return;
        };
        // `HEAD` 中没有的文件：暂存区中有内容时是新增，全部取消暂存后恢复为未跟踪。
        let fresh = entry.untracked() || entry.staged == Some(Change::Added);
        let staged = !line_hunks(head, &next).is_empty();
        let unstaged = !line_hunks(&next, work).is_empty();
        shown.staged = match (staged, fresh) {
            (false, _) => None,
            (true, true) => Some(Change::Added),
            (true, false) => Some(Change::Modified),
        };
        shown.unstaged = match (unstaged, fresh && !staged) {
            (_, true) => Some(Change::Untracked),
            (true, false) => Some(Change::Modified),
            (false, false) => None,
        };
        let path = entry.path.clone();
        self.git.changes.index_changed([path.clone()]);
        self.push_staging(StageJob::Part {
            path,
            hunk,
            line,
            on,
        });
    }

    /// 打开或关闭暂存区视图：与设置中「Git」一页的「显示暂存区」是同一项（`git.stagingArea`）。
    pub(in crate::app) fn toggle_staging_area(&mut self) {
        self.settings_toggle(crate::layout::Setting::GitStagingArea);
    }

    /// 按设置使提交视图成为暂存区视图或勾选式。
    ///
    /// 两种方式的记录互不相通：切换后勾选回到默认，暂存区中已有的内容不变。
    pub(in crate::app) fn apply_staging_view(&mut self) {
        let on = self.config.git_staging_area != Some(false);
        self.git.commit.set_staging(on);
        // 没有选过「显示全部仓库」时它随样式开关：换到列出其他仓库的样式后读它们的状态。
        if self.commit_shows_others() {
            self.git_read_summaries();
        }
        self.git.commit.scroll = self.git.commit.scroll.min(self.commit_max_scroll());
        self.request_redraw();
    }

    /// 暂存全部已跟踪文件的更改（`on` 为真），或者把暂存区中的全部内容移出暂存区。
    ///
    /// 未跟踪的文件不随「暂存全部」加入：它们只在逐个勾选后进入暂存区。
    pub(in crate::app) fn stage_all(&mut self, on: bool) {
        let entries = self.git.status.iter().flat_map(|s| &s.entries);
        let paths: Vec<PathBuf> = entries
            .filter(|e| {
                if on {
                    !e.untracked() && e.staged != Some(Change::Deleted)
                } else {
                    e.staged.is_some()
                }
            })
            .map(|e| e.path.clone())
            .collect();
        self.stage_paths(&paths, on);
    }

    /// 暂存当前标签中的文件（差异标签取其对应的文件）；该文件没有更改时在状态栏说明。
    pub(in crate::app) fn stage_current_file(&mut self) {
        let path = self.editor.active().and_then(|doc| {
            let diff = doc.diff.as_deref().and_then(|view| view.repo_path.clone());
            diff.or_else(|| doc.path.clone())
        });
        let changed = |path: &PathBuf| {
            let mut entries = self.git.status.iter().flat_map(|s| &s.entries);
            entries.any(|e| e.path == *path && e.unstaged.is_some()) || self.stage_buffer(path)
        };
        match path.filter(changed) {
            Some(path) => self.stage_paths(&[path], true),
            None => self.flash(t!("git-stage-nothing")),
        }
    }

    fn stage_buffer(&self, path: &Path) -> bool {
        self.git
            .root
            .as_ref()
            .is_some_and(|root| path.starts_with(root))
            && !self.git.is_ignored(path)
            && self
                .git_document(path)
                .is_some_and(|doc| self.pending_git_save(doc))
            && path.parent().is_some_and(|parent| {
                winc_editor::git::find_root(parent).as_ref() == self.git.root.as_ref()
            })
    }

    pub(super) fn pending_git_save(&self, doc: &winc_editor::Document) -> bool {
        doc.is_dirty() || self.saving(doc.id) || self.format.saving_document(doc.id)
    }

    /// 返回文件树、标签的右键菜单中针对文件 `path` 的更改的几项：回滚，暂存区视图中另有暂存、
    /// 取消暂存。
    ///
    /// 该文件没有更改时为空；已全部暂存的文件没有「暂存」，没有暂存内容的文件没有「取消暂存」。
    pub(in crate::app) fn git_change_items(&self, path: &Path) -> Vec<MenuEntry> {
        let mut entries = self
            .git
            .status
            .iter()
            .flat_map(|s| &s.entries)
            .filter(|e| e.path == path)
            .peekable();
        if !self.git.commit.staging() {
            return if entries.peek().is_some() {
                vec![MenuEntry::Item(MenuCommand::GitRollbackFile, true)]
            } else {
                Vec::new()
            };
        }
        let (mut stage, mut unstage, mut rollback) = (self.stage_buffer(path), false, false);
        for e in entries {
            stage |= e.unstaged.is_some();
            unstage |= CommitGroup::Staged.holds(e, true);
            rollback |= e.unstaged.is_some() && !e.conflicted();
        }
        let mut items = Vec::new();
        if stage {
            items.push(MenuEntry::Item(MenuCommand::CommitCheck(true), true));
        }
        if unstage {
            items.push(MenuEntry::Item(MenuCommand::CommitCheck(false), true));
        }
        if rollback {
            items.push(MenuEntry::Item(
                MenuCommand::GitRollbackFile,
                !self.rollback_busy(),
            ));
        }
        items
    }

    pub(super) fn stage_for_commit(&mut self, request: CommitRequest) {
        self.git.commit.commit_queued = Some(request);
        self.push_staging(StageJob::AddAll);
    }

    /// 把 `job` 排到写入暂存区的队列末尾并开始执行；相邻的同类文件操作合并为一步。
    fn push_staging(&mut self, job: StageJob) {
        self.git.commit.index_changed();
        self.git.commit.status_outdated = self.git.running.is_some();
        // 文件所在的组可能变化（未跟踪的文件暂存后是新增的文件），各行需要重新排列。
        self.git.generation += 1;
        let queue = &mut self.git.commit.staging_queue;
        match (queue.back_mut(), job) {
            (Some(StageJob::Add(last)), StageJob::Add(more))
            | (Some(StageJob::Remove(last)), StageJob::Remove(more)) => last.extend(more),
            (_, job) => queue.push_back(job),
        }
        self.pump_staging();
        self.request_redraw();
    }

    /// 队列中有等待的操作、且没有操作正在执行时，在后台执行下一步。
    fn pump_staging(&mut self) {
        if self.git.commit.staging_running || self.git.root.is_none() {
            return;
        }
        let Some(job) = self.git.commit.staging_queue.pop_front() else {
            return;
        };
        self.git.commit.staging_running = true;
        let sender = self.git.commit.results.0.clone();
        self.run_git_job(move |git, root| {
            let result = git.and_then(|git| match job {
                StageJob::Add(paths) => git.stage(root, &paths).map(drop),
                StageJob::AddAll => git.stage_all_if_empty(root),
                StageJob::Remove(paths) => git.unstage(root, &paths),
                StageJob::Part {
                    path,
                    hunk,
                    line,
                    on,
                } => write_part(git, root, &path, (&hunk, line), on),
            });
            let _ = sender.send(Outcome::Staged(result, root.to_path_buf()));
        });
    }

    /// 写入暂存区的一步操作结束：继续下一步；全部完成后重新读取状态，并执行等待中的提交。
    pub(super) fn staging_finished(&mut self, result: Result<(), String>, root: &Path) {
        if self.git.root.as_deref() != Some(root) {
            return;
        }
        self.git.commit.staging_running = false;
        if let Err(e) = result {
            // 界面已按成功的结果显示：放弃其余的操作，按仓库的实际状态重新显示。
            self.git.commit.staging_queue.clear();
            self.git.commit.commit_queued = None;
            self.notify(t!("git-stage-failed", error = e));
        }
        if !self.git.commit.staging_queue.is_empty() {
            self.pump_staging();
            return;
        }
        self.git_refresh();
        if let Some(request) = self.git.commit.commit_queued.take() {
            self.git_commit_request(request);
        }
    }
}

/// 把 `entry` 改写为暂存（`on` 为真）或取消暂存之后的状态，要写入暂存区的路径加到 `touched`。
///
/// 已经处于该状态的文件不动；冲突中的文件不能取消暂存。
pub(super) fn flip_staged(entry: &mut Entry, on: bool, touched: &mut Vec<PathBuf>) {
    if on {
        let Some(unstaged) = entry.unstaged else {
            return;
        };
        entry.staged = Some(match (entry.staged, unstaged) {
            (_, Change::Deleted) => Change::Deleted,
            (_, Change::Renamed) => Change::Renamed,
            (Some(Change::Added), _) | (_, Change::Untracked | Change::Added) => Change::Added,
            (Some(Change::Renamed), _) => Change::Renamed,
            _ => Change::Modified,
        });
        entry.unstaged = None;
        touched.push(entry.path.clone());
        if unstaged == Change::Renamed {
            touched.extend(entry.from.clone());
        }
    } else {
        let Some(staged) = entry.staged.filter(|_| !entry.conflicted()) else {
            return;
        };
        entry.unstaged = Some(match (staged, entry.unstaged) {
            (Change::Added, _) => Change::Untracked,
            (Change::Deleted, _) | (_, Some(Change::Deleted)) => Change::Deleted,
            _ => Change::Modified,
        });
        entry.staged = None;
        touched.push(entry.path.clone());
        // 仅暂存侧改名时一并取消原路径的暂存；工作区侧改名保留在磁盘上。
        if staged == Change::Renamed {
            touched.extend(entry.from.clone());
        }
    }
}

/// 按磁盘上的内容暂存或取消暂存文件 `path` 的一段改动（或其中一行），写入暂存区。
///
/// 新文件的内容全部取消暂存后移出暂存区，恢复为未跟踪。
fn write_part(
    git: &Git,
    root: &Path,
    path: &Path,
    (hunk, line): (&LineHunk, Option<StageLine>),
    on: bool,
) -> Result<(), String> {
    let cancel = Cancel::default();
    let name = winc_editor::display_name(path).into_owned();
    let head = git.head_content(root, path, &cancel)?;
    let index = git.index_content(root, path, &cancel)?;
    git.check_partial_path(root, path)?;
    let disk = winc_backend::fs::read(path)
        .map_err(|e| t!("git-read-file-failed", name = &name, error = e.to_string()))?;
    let fresh = head.is_none();
    let text = |bytes: Vec<u8>| {
        String::from_utf8(bytes).map_err(|_| t!("git-stage-partial-not-utf8", name = &name))
    };
    let (head, disk) = (text(head.unwrap_or_default())?, text(disk)?);
    let index = match index {
        Some(bytes) => text(bytes)?,
        None => String::new(),
    };
    // 暂存的差异是暂存区到工作区（上次提交一侧即暂存区），取消暂存的是上次提交到暂存区（工作区
    // 一侧即暂存区）：两者都只需其中的两份文本。
    let next = if on {
        stage(&index, &index, &disk, hunk, line)
    } else {
        unstage(&head, &index, &index, hunk, line)
    };
    if fresh && next.is_empty() {
        git.check_partial_path(root, path)?;
        return git.unstage(root, &[path.to_path_buf()]);
    }
    if next == index {
        return Ok(());
    }
    git.stage_content(root, path, next.into_bytes())
}

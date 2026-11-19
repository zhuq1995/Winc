//! 改动列表：行、勾选、选中、滚动、固定在顶部的行和键盘。

use super::*;
use crate::app::ScrollTarget;
use crate::app::git::Compare;

impl App {
    /// 提交视图列表现在的各行（见 [`CommitRows`]）。
    ///
    /// 按状态的版本（`GitState::generation`，换仓库时也变）和收起、平铺的样子缓存：每帧绘制、
    /// 滚动、悬停都要用，不再每次从头排。
    pub(crate) fn commit_rows(&self) -> Rc<CommitRows> {
        rows_of(
            &self.git.commit,
            self.git.status.as_ref(),
            self.git.root.as_deref(),
            self.git.generation,
        )
    }

    /// 状态里第 `i` 个文件；状态里没有这一项时为 `None`。
    pub(super) fn commit_entry(&self, i: usize) -> Option<&Entry> {
        self.git.status.as_ref()?.entries.get(i)
    }

    /// 暂存或取消暂存第 `i` 个文件；该文件在多选的选区里时，选中的各个文件一起操作。
    ///
    /// 只暂存了一部分的文件按未暂存处理：点击后暂存其余的改动。
    pub(super) fn toggle_commit_check(&mut self, i: usize) {
        let Some(entry) = self.commit_entry(i).cloned() else {
            return;
        };
        let on = !self.git.commit.checked(&entry);
        if self.commit_multi_selected() && self.commit_row_selected(&entry.path, None) {
            let paths = self.commit_selection();
            self.set_commit_checked(&paths, on);
        } else {
            self.set_commit_checked(&[entry.path], on);
        }
    }

    /// 只选中 `path`（清掉多选），按 `compare` 在预览标签里打开它的差异；列表滚到让它露出来。
    ///
    /// 浏览历史提交的文件时（`commit.browse`），在该提交的差异标签中打开这个文件。
    pub(in crate::app::git) fn select_commit_file(&mut self, path: PathBuf, compare: Compare) {
        let group = self.git.status.as_ref().and_then(|status| {
            CommitGroup::ALL.into_iter().find(|&group| {
                self.commit_compare(Some(group)) == compare
                    && status
                        .entries
                        .iter()
                        .any(|e| e.path == path && group.holds(e, self.git.commit.staging()))
            })
        });
        self.select_commit_row(path, group, compare);
    }

    pub(super) fn select_commit_row_file(&mut self, path: PathBuf, group: Option<CommitGroup>) {
        self.select_commit_row(path, group, self.commit_compare(group));
    }

    fn select_commit_row(&mut self, path: PathBuf, group: Option<CommitGroup>, compare: Compare) {
        self.git.commit.select_only(path.clone());
        self.set_commit_current(path.clone(), group, true);
        self.reveal_commit_selection();
        if self.git.commit.browse {
            let index = (self.git.status.as_ref())
                .and_then(|status| status.entries.iter().position(|e| e.path == path));
            if let Some(index) = index {
                self.open_log_diff_file(index);
            }
            return;
        }
        self.preview_changes(&path, compare);
    }

    /// 浏览历史提交的文件时点击了第 `i` 个文件：选中它，并在差异标签中打开。
    pub(super) fn browse_commit_file(&mut self, i: usize) {
        let Some(path) = self.commit_entry(i).map(|e| e.path.clone()) else {
            return;
        };
        self.focus = Focus::CommitList;
        self.diff.land = None;
        let compare = self.commit_compare_for(&path);
        self.select_commit_file(path, compare);
    }

    /// 列表里看得见的文件，按显示顺序。
    pub(crate) fn commit_files(&self) -> Vec<PathBuf> {
        let Some(status) = self.git.status.as_ref() else {
            return Vec::new();
        };
        self.commit_rows()
            .iter()
            .filter_map(|r| match r {
                CommitRow::File { index, .. } => Some(status.entries[*index].path.clone()),
                _ => None,
            })
            .collect()
    }

    /// 选中的各个文件，按显示顺序；没有选中时为空。
    pub(crate) fn commit_selection(&self) -> Vec<PathBuf> {
        self.commit_selected_paths(None)
    }

    /// 变更动作按发生操作的组取选区，文件操作按路径去重。
    pub(in crate::app) fn commit_selected_paths(&self, only: Option<CommitGroup>) -> Vec<PathBuf> {
        let Some(status) = self.git.status.as_ref() else {
            return Vec::new();
        };
        let rows = self.commit_rows();
        let mut seen = HashSet::new();
        let selected = |path: &Path, group: Option<CommitGroup>| {
            only.is_none_or(|only| group == Some(only)) && self.commit_row_selected(path, group)
        };
        let mut paths = Vec::new();
        for (row, item) in rows.iter().enumerate() {
            if let CommitRow::File { index, .. } = item {
                let path = &status.entries[*index].path;
                if selected(path, rows.group_of(row)) && seen.insert(path) {
                    paths.push(path.clone());
                }
            }
        }
        if self.commit_marked_count() > 0 || only.is_some() {
            for e in &status.entries {
                if CommitGroup::ALL.into_iter().any(|group| {
                    group.holds(e, self.git.commit.staging()) && selected(&e.path, Some(group))
                }) && seen.insert(&e.path)
                {
                    paths.push(e.path.clone());
                }
            }
        }
        paths
    }

    /// 第 `row` 行（目录）下面的各个文件，收起的子目录里的也算；不是目录时为空。
    pub(crate) fn commit_dir_files(&self, row: usize) -> Vec<PathBuf> {
        let rows = self.commit_rows();
        let (Some(CommitRow::Dir { group, dir, .. }), Some(status)) =
            (rows.get(row), self.git.status.as_ref())
        else {
            return Vec::new();
        };
        rows.dir(*group, dir)
            .iter()
            .map(|&i| status.entries[i].path.clone())
            .collect()
    }

    /// 只选中 `files`（不打开预览），第一个成为当前行和扩展选区的起点。
    pub(crate) fn select_commit_files(&mut self, files: Vec<PathBuf>, group: Option<CommitGroup>) {
        let Some(first) = files.first().cloned() else {
            return;
        };
        self.git.commit.marked.clear();
        let group = self.commit_selection_group(group);
        self.git
            .commit
            .marked
            .insert(group, files.into_iter().collect());
        self.set_commit_current(first, group, true);
    }

    fn commit_selection_group(&self, group: Option<CommitGroup>) -> Option<CommitGroup> {
        group.filter(|_| self.git.commit.staging())
    }

    pub(in crate::app) fn set_commit_current(
        &mut self,
        path: PathBuf,
        group: Option<CommitGroup>,
        anchor: bool,
    ) {
        let group = self.commit_selection_group(group);
        if anchor {
            self.git.commit.anchor = Some((group, path.clone()));
        }
        self.git.commit.selected_group = group;
        self.git.commit.selected = Some(path);
    }

    /// 切换目录中的各行，包括收起的子目录。
    pub(super) fn toggle_commit_dir_selected(&mut self, row: usize) {
        let group = self.commit_rows().group_of(row);
        self.toggle_commit_members(self.commit_dir_files(row), group);
    }

    /// 多选时选中的行数。
    pub(crate) fn commit_marked_count(&self) -> usize {
        self.git.commit.marked.values().map(HashSet::len).sum()
    }

    /// 同一路径只有它所在的选中组显示选中色。
    pub(crate) fn commit_row_selected(&self, path: &Path, group: Option<CommitGroup>) -> bool {
        let commit = &self.git.commit;
        let group = self.commit_selection_group(group);
        if commit.marked.is_empty() {
            return commit.selected.as_deref() == Some(path) && commit.selected_group == group;
        }
        commit
            .marked
            .get(&group)
            .is_some_and(|paths| paths.contains(path))
    }

    /// 是否选中了不止一个文件（还在改动里的，收起的目录里的也算）。
    pub(crate) fn commit_multi_selected(&self) -> bool {
        let Some(status) = self.git.status.as_ref() else {
            return false;
        };
        self.commit_marked_count() > 1
            && status
                .entries
                .iter()
                .filter(|e| {
                    self.git.commit.marked.iter().any(|(group, paths)| {
                        paths.contains(&e.path) && group.is_none_or(|g| g.holds(e, true))
                    })
                })
                .nth(1)
                .is_some()
    }

    pub(super) fn toggle_commit_members(
        &mut self,
        files: Vec<PathBuf>,
        group: Option<CommitGroup>,
    ) {
        let Some(first) = files.first().cloned() else {
            return;
        };
        let group = self.commit_selection_group(group);
        let commit = &mut self.git.commit;
        if commit.marked.is_empty()
            && let Some(current) = commit.selected.clone()
        {
            commit
                .marked
                .entry(commit.selected_group)
                .or_default()
                .insert(current);
        }
        let marked = commit.marked.entry(group).or_default();
        if files.iter().all(|p| marked.contains(p)) {
            for file in &files {
                marked.remove(file);
            }
            commit.marked.retain(|_, paths| !paths.is_empty());
            let rows = self.commit_rows();
            let current = rows.iter().enumerate().find_map(|(row, item)| {
                let CommitRow::File { index, .. } = item else {
                    return None;
                };
                let path = &self.commit_entry(*index)?.path;
                let group = rows.group_of(row);
                self.commit_row_selected(path, group)
                    .then(|| (path.clone(), group))
            });
            if let Some((path, group)) = current.filter(|_| self.commit_marked_count() > 0) {
                self.set_commit_current(path, group, true);
            } else if let Some((group, path)) =
                self.git.commit.marked.iter().find_map(|(group, paths)| {
                    paths.iter().min().map(|path| (*group, path.clone()))
                })
            {
                self.set_commit_current(path, group, true);
            } else {
                self.git.commit.clear_selection();
            }
        } else {
            marked.extend(files);
            self.set_commit_current(first, group, true);
        }
    }

    /// 选中从扩展选区的起点到给定文件行之间的各行。
    pub(super) fn select_commit_range(&mut self, index: usize, group: Option<CommitGroup>) {
        let rows = self.commit_rows();
        let row = rows.iter().enumerate().position(|(r, item)| {
            matches!(item, CommitRow::File { index: i, .. } if *i == index)
                && group.is_none_or(|group| rows.group_of(r) == Some(group))
        });
        if let Some(row) = row {
            self.select_commit_rows(row);
        }
    }

    /// 选中从扩展选区的起点到第 `to` 行之间的各行：文件行是它自己，目录行是它下面的所有文件
    /// （收起的也算）。
    ///
    /// 第 `to` 行的文件（目录时是它下面的第一个）成为当前行。
    pub(super) fn select_commit_rows(&mut self, to: usize) {
        let rows = self.commit_rows();
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        let file_at = |r: usize| match rows.get(r) {
            Some(CommitRow::File { index, .. }) => Some(status.entries[*index].path.clone()),
            _ => None,
        };
        let group_at = |r| self.commit_selection_group(rows.group_of(r));
        let from = self
            .git
            .commit
            .anchor
            .as_ref()
            .and_then(|(group, path)| {
                (0..rows.len())
                    .find(|&r| group_at(r) == *group && file_at(r).as_ref() == Some(path))
            })
            .unwrap_or(to);
        let mut marked: HashMap<_, HashSet<_>> = HashMap::new();
        for r in from.min(to)..=from.max(to) {
            if let Some(path) = file_at(r) {
                marked.entry(group_at(r)).or_default().insert(path);
            } else if let CommitRow::Dir { .. } = rows[r] {
                marked
                    .entry(group_at(r))
                    .or_default()
                    .extend(self.commit_dir_files(r));
            }
        }
        let current = file_at(to).or_else(|| self.commit_dir_files(to).into_iter().next());
        let Some(current) = current else {
            return;
        };
        let (from_group, to_group) = (group_at(from), group_at(to));
        self.git.commit.anchor =
            Some((from_group, file_at(from).unwrap_or_else(|| current.clone())));
        self.git.commit.marked = marked;
        self.set_commit_current(current, to_group, false);
        self.reveal_commit_selection();
    }

    /// 让选中的文件露在列表里：盖在固定的组、目录下面、或者已滚出列表时，滚到它正好露出来。
    pub(in crate::app) fn reveal_commit_selection(&mut self) {
        // 当前仓库一节收起着：选中的文件不在列表里，不滚动（键盘移动选中时先展开，见
        // `App::unfold_current_section`）。
        if self.current_section_folded() {
            return;
        }
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        let rows = self.commit_rows();
        let selected = self.git.commit.selected.as_deref();
        let Some(i) = rows.iter().enumerate().position(|(row, r)| match r {
            CommitRow::File { index, .. } => {
                Some(status.entries[*index].path.as_path()) == selected
                    && self.commit_selection_group(rows.group_of(row))
                        == self.git.commit.selected_group
            }
            _ => false,
        }) else {
            return;
        };
        let list = self.commit_parts().list;
        // 多仓库时改动列表排在其他仓库的各行和当前仓库的标题行下面。
        let before = self.commit_rows_before() as f32 * COMMIT_ROW;
        let (top, bottom) = (
            before + i as f32 * COMMIT_ROW,
            before + (i + 1) as f32 * COMMIT_ROW,
        );
        // 只在固定行盖住该行时才滚：滚到该行正好露在它所在的组、目录（层数对应的行数，不超过
        // 能固定的层数）下面。已经露着的不动，单击列表不会跳、顶上固定的行也不会变。
        let covered = self
            .commit_sticky(&rows)
            .iter()
            .map(|s| s.rect.bottom() - list.y)
            .fold(0.0, f32::max);
        let before = self.git.commit.scroll;
        if top - self.git.commit.scroll < covered {
            let pinned = rows[i].level().min(self.commit_sticky_max()) as f32 * COMMIT_ROW;
            self.git.commit.scroll = (top - pinned).max(0.0);
        } else if top < self.git.commit.scroll {
            self.git.commit.scroll = top;
        } else if bottom > self.git.commit.scroll + list.h {
            self.git.commit.scroll = bottom - list.h;
        }
        self.glide_pane(ScrollTarget::Commit, before);
    }

    /// 第 `parent` 行的组或目录刚收起或展开时，开始展开或收起的动画。
    ///
    /// `before` 是变化之前的各行。子行从第 `parent` 行下方逐渐显示（或收回），其后的各行随之移动。
    ///
    /// 动态效果关闭时直接到位。
    pub(super) fn start_commit_motion(&mut self, before: Rc<CommitRows>, parent: usize) {
        let after = self.commit_rows();
        let expanding = after.len() > before.len();
        let count = after.len().abs_diff(before.len());
        if count == 0 {
            self.git.commit.motion = None;
            return;
        }
        // 动画期间按行数较多的状态画：展开时画展开后的行，收起时画收起前的行（收回的行还在）。
        self.git.commit.fold_motion = None;
        let rows = if expanding { after } else { before };
        self.git.commit.motion = self
            .clock(crate::app::animation::TREE_ANIMATION)
            .map(|clock| CommitMotion {
                rows,
                parent,
                count,
                expanding,
                clock,
            });
    }

    /// 最多固定几层：[`STICKY_MAX`]，也不超过列表高度的四成。
    fn commit_sticky_max(&self) -> usize {
        let list = self.commit_parts().list;
        ((list.h * 0.4 / COMMIT_ROW).floor() as usize).min(STICKY_MAX)
    }

    /// 固定在列表顶部的组、目录，外层在前（规则与 `tree_sticky` 相同）。
    ///
    /// 逐层往下找：已经固定了 n 层时，看紧挨着这些层下面露出的那一行。它在第 n 层的上级（组或
    /// 目录）是下一个固定的；它自己就是第 n 层的组、目录、而且上沿已经滚到固定的位置之上时，
    /// 固定它自己。这一层最后一行的下沿滚到固定的位置之下时，它被往上推，下面不再固定别的。
    pub(crate) fn commit_sticky(&self, rows: &[CommitRow]) -> Vec<CommitSticky> {
        let list = self.commit_parts().list;
        let scroll = self.commit_block_scroll();
        let max = self.commit_sticky_max();
        let mut stack: Vec<CommitSticky> = Vec::new();
        while stack.len() < max {
            let level = stack.len();
            let top = list.y + level as f32 * COMMIT_ROW;
            let first = ((scroll + level as f32 * COMMIT_ROW) / COMMIT_ROW).floor() as usize;
            let Some(row) = rows.get(first) else {
                break;
            };
            let row_top = list.y + first as f32 * COMMIT_ROW - scroll;
            let index = if row.level() > level {
                match rows[..first].iter().rposition(|r| r.level() <= level) {
                    Some(i) if rows[i].level() == level && rows[i].is_container() => i,
                    _ => break,
                }
            } else if row.level() == level && row.is_container() && row_top < top - 0.5 {
                first
            } else {
                break;
            };
            let count = rows[index + 1..]
                .iter()
                .take_while(|r| r.level() > level)
                .count();
            if count == 0 {
                break;
            }
            let bottom = list.y + (index + count + 1) as f32 * COMMIT_ROW - scroll;
            let y = top.min(bottom - COMMIT_ROW);
            stack.push(CommitSticky {
                row: index,
                level,
                rect: Rect::new(list.x, y, list.w, COMMIT_ROW),
            });
            if y < top {
                break;
            }
        }
        stack
    }

    /// 返回选中文件在 `rows` 中的行号。
    ///
    /// 选中文件位于收起的目录（或组）中而不可见时，返回包含该文件的最内层收起目录（或组）的行号。
    /// 没有选中文件时为 `None`。
    fn commit_anchor(&self, rows: &CommitRows) -> Option<usize> {
        let (status, root) = (self.git.status.as_ref()?, self.git.root.as_deref()?);
        let selected = self.git.commit.selected.as_deref()?;
        let group = self.git.commit.selected_group;
        let index = status
            .entries
            .iter()
            .position(|e| e.path == selected && group.is_none_or(|group| group.holds(e, true)))?;
        let entry = &status.entries[index];
        // 目录排在其子目录之前，因此最后一个包含该文件的行即最内层。
        rows.iter().enumerate().rposition(|(i, row)| {
            group.is_none_or(|group| rows.group_of(i) == Some(group))
                && match row {
                    CommitRow::File { index: i, .. } => *i == index,
                    CommitRow::Group(group) => group.holds(entry, self.git.commit.staging()),
                    CommitRow::Dir { group, dir, .. } => {
                        group.holds(entry, self.git.commit.staging())
                            && entry.path.starts_with(root.join(dir))
                    }
                }
        })
    }

    /// 以动画收起或展开第 `row` 行（组或目录），滚动位置不超过新的末尾。
    /// 递归收起或展开第 `row` 行的目录（`⌥` 单击）：展开的目录连同各级子目录一起收起，收起的
    /// 目录展开到底。只改内存中的记录，不访问磁盘。
    pub(super) fn toggle_commit_dir_recursively(&mut self, row: usize) {
        let before = self.commit_rows();
        let (Some(CommitRow::Dir { group, dir, .. }), Some(status), Some(root)) = (
            before.get(row),
            self.git.status.as_ref(),
            self.git.root.as_deref(),
        ) else {
            return;
        };
        let collapse = !self.git.commit.dir_collapsed(*group, dir);
        self.git
            .commit
            .set_dir_tree_collapsed(status, root, (*group, dir), collapse);
        self.start_commit_motion(Rc::clone(&before), row);
        self.git.commit.scroll = self.git.commit.scroll.min(self.commit_max_scroll());
    }

    pub(super) fn toggle_commit_row(&mut self, row: usize) {
        let before = self.commit_rows();
        match before.get(row) {
            Some(CommitRow::Group(group)) => self.git.commit.toggle_group(*group),
            Some(CommitRow::Dir { group, dir, .. }) => {
                self.git.commit.toggle_dir(*group, dir.clone());
            }
            _ => return,
        }
        self.start_commit_motion(before, row);
        self.git.commit.scroll = self.git.commit.scroll.min(self.commit_max_scroll());
    }

    /// 对更改列表中选中的文件执行 `action`（显示差异、打开源文件、回滚），返回是否已处理。
    ///
    /// 焦点不在更改列表上、正在浏览历史提交的文件时不处理，按键继续交给其他绑定。
    pub(in crate::app) fn run_changes_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        if self.focus != Focus::CommitList || self.log_diff_tree().is_some() {
            return false;
        }
        let Some(current) = self.git.commit.selected.clone() else {
            return true;
        };
        let paths = self.commit_selection();
        match action {
            // 选中时差异已在预览标签中：焦点移过去；尚未打开时先打开。
            Action::ChangesShowDiff => {
                let shown = (self.editor.active())
                    .and_then(|doc| doc.diff.as_deref())
                    .is_some_and(|view| view.repo_path.as_deref() == Some(current.as_path()));
                if shown {
                    self.focus = Focus::Editor;
                } else {
                    self.select_commit_row_file(current, self.git.commit.selected_group);
                }
            }
            Action::ChangesOpenSource => {
                let files: Vec<&PathBuf> = (paths.iter())
                    .filter(|path| winc_backend::fs::is_file(path))
                    .collect();
                for path in &files {
                    self.open_path(path);
                }
                if !files.is_empty() {
                    self.focus = Focus::Editor;
                }
            }
            _ => self.rollback_commit_selection(),
        }
        self.request_redraw();
        true
    }

    fn rollback_commit_selection(&mut self) {
        let group = self.git.commit.selected_group;
        if !self.git.commit.staging()
            || matches!(group, Some(CommitGroup::Changes | CommitGroup::Untracked))
        {
            self.confirm_rollback(self.commit_selected_paths(group));
        }
    }

    /// 处理改动列表中的按键。
    ///
    /// `↑` `↓` 选择并预览，`Home` `End` 移到第一个、最后一个文件，`PageUp` `PageDown` 翻页，
    /// 以上按键配合 `⇧` 时扩展选区；`←` `→` 收起、展开选中文件所在的目录；空格切换勾选；
    /// `⌫` 回滚选中的文件；回车打开文件；`Tab` 移到提交信息；`Esc` 先缩小为单选，再回到编辑区。
    pub(in crate::app) fn on_key_commit_list(&mut self, event: &KeyEvent) {
        self.commit_list_key(&event.logical_key);
    }

    /// [`App::on_key_commit_list`] 按下的是 `key` 时做的事。
    ///
    /// 浏览历史提交的文件时只处理移动和展开收起：勾选、回滚、打开工作区文件都不适用。
    pub(in crate::app) fn commit_list_key(&mut self, key: &Key) {
        if self.git.commit.browse
            && matches!(
                key,
                Key::Named(
                    NamedKey::Space
                        | NamedKey::Backspace
                        | NamedKey::Delete
                        | NamedKey::Enter
                        | NamedKey::Tab
                )
            )
        {
            return;
        }
        let rows = self.commit_rows();
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        // 各文件在 `rows` 中的行号及其在状态中的下标。
        let files: Vec<(usize, usize)> = rows
            .iter()
            .enumerate()
            .filter_map(|(r, row)| match row {
                CommitRow::File { index, .. } => Some((r, *index)),
                _ => None,
            })
            .collect();
        let selected = self.git.commit.selected.as_deref();
        let group = self.git.commit.selected_group;
        let at = files.iter().position(|&(r, i)| {
            Some(status.entries[i].path.as_path()) == selected
                && self.commit_selection_group(rows.group_of(r)) == group
        });
        // 选中文件所在的行；文件被收起而不可见时为收起它的目录（或组）所在的行。
        let anchor = self.commit_anchor(&rows);
        // 要用到的那个文件的路径先取出来，下面改 `self` 时不再借着状态。
        let path = |i: usize| files.get(i).map(|&(_, f)| status.entries[f].path.clone());
        // 翻页距离为可见整行数减一，保留一行上下文；落在该行或其后第一个文件上，
        // 跳过组、目录行，到达两端时停在第一个、最后一个文件。
        let page = ((self.commit_parts().list.h / COMMIT_ROW).floor() as usize)
            .saturating_sub(1)
            .max(1);
        let last = files.len().checked_sub(1);
        let paged = |i: usize, down: bool| {
            let row = files[i].0;
            if down {
                let goal = row + page;
                files.iter().position(|&(r, _)| r >= goal).or(last)
            } else {
                let goal = row.saturating_sub(page);
                files.iter().rposition(|&(r, _)| r <= goal).or(Some(0))
            }
        };
        let moved = match (key, at) {
            // 选中文件已被收起：从收起它的行开始向下或向上查找。
            (Key::Named(NamedKey::ArrowDown), None) if anchor.is_some() => {
                files.iter().position(|&(r, _)| Some(r) > anchor)
            }
            (Key::Named(NamedKey::ArrowUp), None) if anchor.is_some() => {
                files.iter().rposition(|&(r, _)| Some(r) < anchor)
            }
            (Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown), None) => Some(0),
            (Key::Named(NamedKey::ArrowDown), Some(i)) => Some(i + 1),
            (Key::Named(NamedKey::ArrowUp), Some(i)) => i.checked_sub(1),
            (Key::Named(NamedKey::Home), _) => Some(0),
            (Key::Named(NamedKey::End), _) => last,
            (Key::Named(NamedKey::PageUp | NamedKey::PageDown), None) => Some(0),
            (Key::Named(NamedKey::PageDown), Some(i)) => paged(i, true),
            (Key::Named(NamedKey::PageUp), Some(i)) => paged(i, false),
            (Key::Named(NamedKey::Enter), Some(i)) => Some(i),
            _ => None,
        };
        let target = moved.and_then(path);
        let row = moved.and_then(|i| files.get(i)).map(|&(row, _)| row);
        if target.is_some() {
            self.unfold_current_section();
        }

        match key {
            Key::Named(
                NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::PageUp
                | NamedKey::PageDown,
            ) => {
                if let (Some(next), Some(row)) = (target, row) {
                    if self.pointer.modifiers.shift_key() && !self.git.commit.browse {
                        self.select_commit_rows(row);
                    } else {
                        self.select_commit_row_file(next, rows.group_of(row));
                    }
                }
            }
            // `←` 收起选中文件所在的目录，再次按下时收起上一级；`→` 逐级展开，直到文件重新可见。
            Key::Named(NamedKey::ArrowLeft) => {
                let parent = anchor.and_then(|a| {
                    let level = rows[a].level();
                    (0..a).rev().find(|&r| rows[r].level() < level)
                });
                if let Some(row) = parent {
                    self.toggle_commit_row(row);
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if let Some(row) = anchor.filter(|_| at.is_none()) {
                    self.toggle_commit_row(row);
                    self.reveal_commit_selection();
                }
            }
            Key::Named(NamedKey::Space) => {
                if self.git.commit.staging() {
                    if let Some(group) = group {
                        let paths = self.commit_selected_paths(Some(group));
                        self.stage_paths(&paths, group != CommitGroup::Staged);
                    }
                } else if let Some(i) = at {
                    self.toggle_commit_check(files[i].1);
                }
            }
            Key::Named(NamedKey::Backspace | NamedKey::Delete) => self.rollback_commit_selection(),
            Key::Named(NamedKey::Enter) => {
                if let Some(path) = target
                    && winc_backend::fs::is_file(&path)
                {
                    self.open_path(&path);
                    self.focus = Focus::Editor;
                }
            }
            // 没有提交卡片（没有更改）时提交信息不在屏幕上，焦点留在列表里。
            Key::Named(NamedKey::Tab) if self.commit_compose_shown() => {
                self.focus = Focus::CommitMessage;
            }
            Key::Named(NamedKey::Escape) if self.commit_multi_selected() => {
                if let Some(path) = self.git.commit.selected.clone() {
                    self.select_commit_row_file(path, group);
                }
            }
            Key::Named(NamedKey::Escape) if self.editor.active.is_some() => {
                self.focus = Focus::Editor;
            }
            _ => {}
        }
        self.request_redraw();
    }
}

/// 按 `state` 的收起、平铺设置排出 `status` 的各行，按（`generation`，排列设置的版本）缓存在
/// `state` 中；没有状态或仓库时返回空的列表。
pub(in crate::app::git) fn rows_of(
    state: &CommitState,
    status: Option<&Status>,
    root: Option<&Path>,
    generation: u64,
) -> Rc<CommitRows> {
    let (Some(status), Some(root)) = (status, root) else {
        return Rc::default();
    };
    let key = (generation, state.layout);
    let mut cache = state.rows.borrow_mut();
    if let Some((built, rows)) = cache.as_ref()
        && *built == key
    {
        return Rc::clone(rows);
    }
    let rows = Rc::new(state.build_rows(status, root));
    *cache = Some((key, Rc::clone(&rows)));
    rows
}

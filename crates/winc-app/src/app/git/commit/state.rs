//! 提交面板里可以收起、展开的样子、按目录分层排好的改动列表，以及提交前的检查。

use super::*;

impl CommitState {
    /// 返回当前按下「提交」不会提交的原因；可以提交（或正在提交）时为 `None`。
    ///
    /// 检查项与 [`App::git_commit`] 相同。提交按钮据此置灰，悬停提示显示原因。
    pub fn blocker(&self, status: &Status) -> Option<String> {
        if self.busy || self.staging {
            return None;
        }
        let rebasing = status.operation == Some(Operation::Rebase);
        let unresolved = (status.entries.iter()).filter(|e| e.conflicted()).count();
        if unresolved > 0 {
            return Some(t!("git-commit-unresolved-conflicts", count = unresolved).to_owned());
        }
        if status.entries.is_empty() && !self.amend && !rebasing {
            return Some(t!("git-commit-nothing-to-commit").to_owned());
        }
        if self.message.text().trim().is_empty() && !rebasing {
            return Some(t!("git-commit-message-required").to_owned());
        }
        None
    }

    /// 把正在写的提交信息记到位置 `root` 名下；什么都没写时清掉记录。
    pub(in crate::app) fn keep_draft(&mut self, root: PathBuf) {
        let text = if self.amend {
            self.draft.clone().unwrap_or_default()
        } else {
            self.message.text().to_owned()
        };
        if text.trim().is_empty() {
            self.drafts.remove(&root);
        } else {
            self.drafts.insert(root, text);
        }
    }

    /// 填回位置 `root` 名下记着的提交信息。
    pub(in crate::app) fn restore_draft(&mut self, root: &Path) {
        if let Some(text) = self.drafts.remove(root) {
            self.message.set_text(&text);
        }
    }

    /// 组 `group` 是否收起。
    pub fn group_collapsed(&self, group: CommitGroup) -> bool {
        self.collapsed[group.index()]
    }

    /// 是否平铺显示（不按目录排成树）。
    pub fn flat(&self) -> bool {
        self.flat
    }

    /// 组 `group` 里的目录 `dir`（相对仓库根目录）是否收起。
    pub fn dir_collapsed(&self, group: CommitGroup, dir: &Path) -> bool {
        self.collapsed_dirs[group.index()].contains(dir)
    }

    /// 收起或展开组 `group`。
    pub(super) fn toggle_group(&mut self, group: CommitGroup) {
        let i = group.index();
        self.collapsed[i] = !self.collapsed[i];
        self.layout += 1;
    }

    /// 收起或展开组 `group` 里的目录 `dir`。
    pub(super) fn toggle_dir(&mut self, group: CommitGroup, dir: PathBuf) {
        let dirs = &mut self.collapsed_dirs[group.index()];
        if !dirs.remove(&dir) {
            dirs.insert(dir);
        }
        self.layout += 1;
    }

    /// 收起（`collapse` 为真）或展开组 `group` 里的目录 `dir` 和它下面的各级目录。
    pub(super) fn set_dir_tree_collapsed(
        &mut self,
        status: &Status,
        root: &Path,
        (group, dir): (CommitGroup, &Path),
        collapse: bool,
    ) {
        // 收起的目录下面的行不会排出来：先全部展开排一遍，才看得到各级目录。
        let collapsed = std::mem::take(&mut self.collapsed_dirs[group.index()]);
        let all = self.build_rows(status, root);
        self.collapsed_dirs[group.index()] = collapsed;
        let under = all.iter().filter_map(|row| match row {
            CommitRow::Dir {
                group: g, dir: d, ..
            } if *g == group && d.starts_with(dir) => Some(d.clone()),
            _ => None,
        });
        let dirs = &mut self.collapsed_dirs[group.index()];
        for path in under {
            if collapse {
                dirs.insert(path);
            } else {
                dirs.remove(&path);
            }
        }
        self.layout += 1;
    }

    /// 设定平铺（`true`）或按目录排成树。
    pub(crate) fn set_flat(&mut self, flat: bool) {
        if self.flat != flat {
            self.flat = flat;
            self.layout += 1;
        }
    }

    /// 全部收起，或者全部展开（连收起的目录一起展开）。
    pub(super) fn collapse_all(&mut self, collapse: bool) {
        self.collapsed = [collapse; CommitGroup::COUNT];
        if !collapse {
            self.forget_dirs();
        }
        self.layout += 1;
    }

    /// 清除收起的目录记录（换了目录，它们是旧仓库的）。
    pub(in crate::app) fn forget_dirs(&mut self) {
        self.collapsed_dirs.iter_mut().for_each(HashSet::clear);
        self.layout += 1;
    }

    /// 排列表：每组一行标题，没收起的组下面按目录排成树（或者平铺）。
    ///
    /// 空的组不显示。`root` 是仓库的根目录，文件的路径按它算相对路径。同一个文件可以同时出现在
    /// 「已暂存的更改」和「更改」里：两组各排各的，行上的组说明它是哪一边。
    pub(super) fn build_rows(&self, status: &Status, root: &Path) -> CommitRows {
        let mut out = CommitRows::default();
        for group in CommitGroup::ALL {
            let files: Vec<(usize, &Path)> = status
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| group.holds(e))
                .map(|(i, e)| (i, e.path.strip_prefix(root).unwrap_or(&e.path)))
                .collect();
            if files.is_empty() {
                continue;
            }
            out.rows.push(CommitRow::Group(group));
            let start = out.members.len();
            if self.collapsed[group.index()] {
                out.members.extend(files.iter().map(|&(index, _)| index));
            } else if self.flat {
                // 平铺：按路径排，每个文件一行。
                let mut files = files;
                files.sort_by(|a, b| natural_path_cmp(a.1, b.1));
                out.push_files(group, &files, 0);
            } else {
                self.push_level(group, Path::new(""), files, 0, &mut out);
            }
            out.groups[group.index()] = start..out.members.len();
        }
        out
    }

    /// 把 `dir` 下面的 `items`（下标、相对 `dir` 的路径）排成一层：先是子目录（未收起的继续
    /// 往下排），再是文件。
    fn push_level<'a>(
        &self,
        group: CommitGroup,
        dir: &Path,
        items: Vec<(usize, &'a Path)>,
        depth: usize,
        out: &mut CommitRows,
    ) {
        let mut files = Vec::new();
        let mut dirs: BTreeMap<NaturalName<'a>, Vec<(usize, &'a Path)>> = BTreeMap::new();
        for (index, path) in items {
            let mut parts = path.components();
            match (parts.next(), parts.as_path()) {
                (Some(first), rest) if !rest.as_os_str().is_empty() => {
                    dirs.entry(NaturalName(first.as_os_str()))
                        .or_default()
                        .push((index, rest));
                }
                _ => files.push((index, path)),
            }
        }
        for (NaturalName(name), mut sub) in dirs {
            // 只有一个子目录、没有文件：和子目录合成一行。
            let mut label = PathBuf::from(name);
            while let Some(next) = only_subdir(&sub) {
                label.push(next);
                for (_, p) in &mut sub {
                    let rest: &'a Path = p;
                    *p = rest.strip_prefix(next).unwrap_or(Path::new(""));
                }
            }
            let full = dir.join(&label);
            let start = out.members.len();
            out.rows.push(CommitRow::Dir {
                group,
                dir: full.clone(),
                // 显示和折叠目录链的代码都按「/」分段：Windows 的路径分隔符在这里换成「/」。
                label: label.to_string_lossy().replace('\\', "/"),
                depth,
            });
            if self.dir_collapsed(group, &full) {
                out.members.extend(sub.iter().map(|&(index, _)| index));
            } else {
                self.push_level(group, &full, sub, depth + 1, out);
            }
            out.dirs[group.index()].insert(full, start..out.members.len());
        }
        files.sort_by(|a, b| natural_path_cmp(a.1, b.1));
        out.push_files(group, &files, depth);
    }
}

/// 两个相对路径按文件视图的自然顺序比（逐级比名字，`2.png` 在 `10.png` 前面）。
fn natural_path_cmp(a: &Path, b: &Path) -> std::cmp::Ordering {
    a.components()
        .zip(b.components())
        .map(|(x, y)| {
            let (x, y) = (
                x.as_os_str().to_string_lossy(),
                y.as_os_str().to_string_lossy(),
            );
            winc_editor::natural_cmp(&x, &y)
        })
        .find(|o| o.is_ne())
        .unwrap_or_else(|| a.components().count().cmp(&b.components().count()))
}

/// `items`（下标、相对路径）都在同一个子目录里、这一层没有文件时，返回那个子目录的名字。
fn only_subdir<'a>(items: &[(usize, &'a Path)]) -> Option<&'a OsStr> {
    let first = |p: &'a Path| {
        let mut parts = p.components();
        let first = parts.next()?.as_os_str();
        (!parts.as_path().as_os_str().is_empty()).then_some(first)
    };
    let next = first(items.first()?.1)?;
    items
        .iter()
        .all(|&(_, p)| first(p) == Some(next))
        .then_some(next)
}

/// 按自然顺序排的目录名，用作 `BTreeMap` 的键。
#[derive(PartialEq, Eq)]
struct NaturalName<'a>(&'a OsStr);

impl Ord for NaturalName<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        winc_editor::natural_cmp(&self.0.to_string_lossy(), &other.0.to_string_lossy())
            .then_with(|| self.0.cmp(other.0))
    }
}

impl PartialOrd for NaturalName<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

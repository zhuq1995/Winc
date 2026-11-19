# Git 暂存与提交：跨层契约

改提交面板、暂存操作、回滚或 Git 差异基准前先读这一页。默认暂存区视图使用真实 Git 索引作为提交来源。

## 1. 适用范围

- `winc-editor::git` 的索引操作、工作区恢复与提交。
- `winc-app` 的改动分组、差异基准、确认归属、保存顺序与写操作队列。

## 2. 签名

```rust
impl Git {
    // crates/winc-editor/src/git.rs
    pub fn stage(&self, root: &Path, paths: &[PathBuf]) -> Result<String, String>;
    pub fn commit_status(&self, root: &Path, cancel: &Cancel) -> Result<Status, String>;

    // crates/winc-editor/src/git/commit.rs
    pub fn unstage(&self, root: &Path, paths: &[PathBuf]) -> Result<(), String>;
    pub fn restore_worktree(&self, root: &Path, paths: &[PathBuf]) -> Result<(), String>;
    pub fn stage_all_if_empty(&self, root: &Path) -> Result<(), String>;
    pub fn commit_with(
        &self, root: &Path, message: &str, options: CommitOptions,
    ) -> Result<String, String>;
}
```

`App::confirmed_commit(&mut self, root: &Path, request: CommitRequest)` 接收发起仓库和提交请求。`CommitRequest` 携带确认范围与推送或同步意图。`App::git_rollback(&mut self, rollback: Rollback)` 接收发起仓库、选择路径和恢复计划。实现位于 `crates/winc-app/src/app/git/commit/actions.rs`。

## 3. 契约

- **行内入口**：组标题、目录、文件行最右是 ＋／−（`GroupStage`／`DirStage`／`StageFile`），悬停时出现；暂存区没有回滚按钮（− 即撤回到更改区），整组回滚只在更改区。命中区域存在 `App::hits.commit`（`crates/winc-app/src/app/hits.rs`），不另建字段。
- **目录批量范围**：`CommitTarget::DirStage(row, on)` 通过 `commit_rows().dir(group, dir)` 取得当前组内的全部成员，包含折叠的子目录；复用 `App::stage_paths` 排队写入。已暂存组使用 −，更改与未跟踪组使用 ＋；浏览历史、勾选式与冲突目录不提供此按钮。绘制后的状态代次变化时拒绝旧命中，名称裁剪、悬停提示与鼠标移入按钮后的命中保持一致。
- **目录长名与固定行**：暂存按钮跟在展开后的目录全名之后；`CommitOverflow::dir_action` 保存终点，列表内外的文字与按钮采用相同的展开进度和裁剪边界。按钮命中先于目录溢出命中；顶部固定目录按固定行的纵向范围裁剪，保留侧栏外按钮的横向命中。
- **行身份**由 `(仓库根目录, CommitGroup, 路径)` 组成。右键、多选、目录和空格键保留组身份，同一路径在两组分别提供暂存和取消暂存。整文件暂存处理工作区侧改名的两个路径，取消暂存处理索引侧改名的两个路径。
- **分组**：合并更改（有冲突时）／已暂存的更改（`staged.is_some()`）／更改（`unstaged.is_some()`）。未跟踪未忽略文件在“更改”。
- **差异基准**：已暂存 = `HEAD` ↔ `index_content`，只读；更改 = `index_content` ↔ 当前文档或磁盘。`HEAD` 不变而 index 变化时也要刷新两侧。
- **比较类型**：`crates/winc-app/src/app/git.rs` 的 `Compare` 是唯一来源——`Working`（暂存区↔工作区：更改组、`git_show_changes`、文件树预览）、`Staged`（`HEAD`↔暂存区：已暂存组）、`Head`（`HEAD`↔工作区：勾选式与冲突中的文件）。差异标签按 `DiffTab::compare` 决定块／行操作写入还是退回暂存区，`DiffTab::index` 记录比较时的暂存区指纹（`App::index_stamp`）。显示暂存区时文件行的命中是 `CommitTarget::FileIn(index, group)`，勾选式仍是 `CommitTarget::File(index)`。行号栏改动标记与回滚以暂存区为基准，界面写入暂存区后调用 `LineChanges::index_changed` 作废。
- **回滚**在默认暂存区视图使用 `Git::restore_worktree`，执行 `git restore --worktree --`，保留索引。暂存新增和暂存改名的文件恢复到索引内容。单项未跟踪文件和暂存删除后同路径重建的工作区文件送废纸篓。目录和组动作按选中组取目标，「放弃所有更改」只收集受跟踪的未暂存改动。勾选式沿用 `Git::rollback` 的 HEAD 基准。
- **路径**通过 `:(literal)` 交给 Git。指定文件的暂存、取消暂存及回滚确认原仓库存在。`check_index_paths` 拒绝替换了文件的普通目录，保留子模块和符号链接行为。`commit_status` 读取完整状态，显示列表按 `STATUS_LIMIT` 截断。`GitStageAll` 在列表范围内暂存受跟踪改动，保留暂存删除及同路径未跟踪文件。
- **字节**以 Git blob 为比较基础，展示文本经过滤器转换。`content_at`／`index_content` 返回检出形式，可与磁盘内容逐行比较。
- **保存**在整文件暂存或空索引提交确认后执行，限定于操作选择和实际所属仓库。保存失败或磁盘冲突时停止写索引。取消暂存只写索引。
- **互斥**由当前项目的暂存队列和提交、分支忙碌状态协调。回滚确认绑定发起仓库和基准，执行前重读目标状态。其他项目或终端对同一索引的写入可与当前项目的操作交错。
- **提交**默认以真实索引为来源，执行 `git commit --file=- --cleanup=whitespace`，修改上次提交时加 `--amend`。普通提交、提交并推送和提交并同步共用范围判定。空索引且有工作区改动时，先确认暂存全部非忽略改动（含未跟踪文件），再保存 → 暂存 → 提交。暂存前若发现已有索引改动，则改用当前索引。取消时保留原状态，提交失败保留索引和提交草稿。
- **差异视图**：`crates/winc-app/src/view/diff.rs` 的 `draw_body` 同时接收上游折叠过渡 `(scroll, shift): (Scroll, Option<&FoldShift>)`、fork 的暂存目标 `file: Option<&CommitFile>` 与差异基准 `compare: Option<Compare>`，`drawn_rows` 用上游签名。横向滚动沿用上游 `DiffHScroll`／`DIFF_HSCROLL`，两侧滑块由同一个滚动位置驱动。
- **文案**：`en-US` 与 `zh-CN` 成对增删；`winc-i18n` 的键一致与未使用消息测试必须保持通过。

## 4. 校验与错误

| 条件 | 结果 |
| --- | --- |
| 选择对应的内容已变化（保存、格式化、外部修改） | 返回选择已变化，刷新差异并提示重新选择；禁止回退为整文件暂存 |
| `index.lock` 被占用 | 返回 Git 的原因；index 不变 |
| 二进制、非 UTF-8、冲突条目 | 明确报错，部分操作不执行 |
| 符号链接或按普通文件检出的 `120000` 记录 | `Git::check_partial_path` 拒绝部分操作（`git-stage-partial-symlink`），整文件操作可用 |
| 暂存目标从文件变为普通目录 | 拒绝整文件操作，保留目录下的文件与索引 |
| 尚无首个提交时取消暂存 | `git rm --cached --force` 移除索引条目，工作区文件保留 |
| 合并／拣选／还原进行中 | 部分操作不可用；提交使用当前 index |
| 提交被钩子或身份拒绝 | 保留 index 与提交信息，显示原因 |
| 项目关闭或换仓库时结果到达 | 按仓库根与请求编号丢弃，不写入当前项目 |
| 回滚目标的仓库、基准或候选范围变化，或目标正在保存 | 停止回滚并提示重新检查 |
| 提交、撤销确认的仓库改变，或撤销目标不再是 HEAD | 停止原操作 |

## 5. 情形

- 好的回滚案例。v1 已提交、v2 已暂存、磁盘为 v3，从「更改」回滚后磁盘与索引均为 v2，HEAD 仍为 v1。提交后 HEAD 为 v2。
- 基准：只暂存一个文件的全部改动；其他已暂存文件保持原状。
- 坏：`git add` 委托给 shell 字符串拼接，路径里的 `[`、`*`、`?` 被当成通配符，连带暂存别的文件。

## 6. 必须的测试

- 提交只含 index：`a_commit_takes_the_index_not_the_worktree`。
- 部分暂存与取消：`partial_staging_writes_only_the_selected_hunks_and_lines`、`unstaging_one_hunk_keeps_the_other_one_staged`。
- 过期选择拒绝：`a_stale_selection_is_refused_instead_of_staging_the_file`。
- 首提交前取消暂存：`unstaging_before_the_first_commit_removes_the_entries`。
- 新增／删除／空文件／改名：`partial_staging_new_deleted_and_empty_files`、`staging_a_rename_covers_the_old_path_too`。
- 字节形态：`partial_staging_keeps_crlf_bom_and_missing_final_newline`。
- 特殊字符路径和路径类型变化：`partial_staging_does_not_touch_files_with_similar_names`、`staging_a_file_replaced_by_a_directory_does_not_stage_its_children`、`partial_staging_refuses_a_link_checked_out_as_a_regular_file`、`a_removed_worktree_does_not_redirect_commits_to_the_parent_repository`。
- 空索引提交的取消、忽略文件范围、索引变化、磁盘冲突、钩子失败和推送／同步续接，运行 `cargo test -p winc-app --lib directory_staging::save_and_commit`。
- 双组身份与唯一基准：`a_file_staged_in_the_terminal_appears_in_both_groups`。
- 目录按钮、组内范围与折叠后批量操作：`crates/winc-app/src/app/snapshot/directory_staging.rs` 在各平台直接编译，使用离屏鼠标点击与真实 Git 索引断言，覆盖首次提交前取消暂存、过期命中、历史与勾选式视图、临界长度与超出侧栏的目录名，以及顶部固定目录的按钮点击。
- 回滚与组选区用 `working_row_rollback_keeps_the_staged_version_and_commit_scope`、`staging_selection_actions_use_the_selected_side_and_keep_unsaved_text`、`directory_menu_and_discard_all_limit_working_tree_scope`、`rollback_confirmation_rejects_a_changed_repo_busy_write_or_stale_trash_candidate` 覆盖。底层用 `restoring_the_worktree_uses_only_the_index_even_before_the_first_commit` 检查无首次提交与字面路径。

测试仓库在 `git init` 后设置 `core.autocrlf=false`，不依赖本机全局配置。

## 7. 错误与正确写法

```rust
// 错误：只用路径当身份，同一文件的两组会互相覆盖。
row(&mut self, path: &Path)

// 正确：组与路径一起作为身份。
row(&mut self, group: CommitGroup, path: &Path)
```

```rust
// 错误：提交时把勾选内容加入 index，改变用户的暂存区。
command.args(["commit", "--only", "--"]).args(paths);

// 正确：提交真实 index，暂存由独立操作完成。
command.args(["commit", "--file=-", "--cleanup=whitespace"]);
```

```rust
// 错误：部分暂存失败后整文件暂存。
if stage_partial(..).is_err() { git.stage(root, &[path]).map(|_| ())?; }

// 正确：返回原因，刷新后让用户重新选择。
stage_partial(root, &path, &content)?;
```

```rust
// 暂存区视图的「更改」回滚，使用索引恢复工作区。
git.restore_worktree(root, &paths)?;
// git.rollback(root, &paths, &added) 仅用于勾选式的 HEAD 回滚。
```

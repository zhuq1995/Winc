# Git 暂存与提交：跨层契约

改提交面板、暂存操作或 Git 差异基准前先读这一页。真实 Git 暂存区是唯一提交来源；界面不维护自己的勾选状态。

## 1. 适用范围

- `winc-editor::git` 的 index 操作与提交。
- `winc-app` 的改动分组、行身份、差异比较类型、保存前置与写操作互斥。
- 触发条件：新增或修改暂存／取消暂存、提交、改动列表分组、Git 差异基准。

## 2. 签名

```rust
// winc-editor/src/git/staging.rs
impl Git {
    // git.rs；返回 git 的输出摘要
    fn stage(&self, root: &Path, paths: &[PathBuf]) -> Result<String, String>;
    fn unstage(&self, root: &Path, paths: &[PathBuf]) -> Result<(), String>;
    fn index_content(&self, root: &Path, path: &Path, cancel: &Cancel)
        -> Result<Option<Vec<u8>>, String>;
    fn stage_partial(&self, root: &Path, path: &Path, content: &[u8]) -> Result<(), String>;
    fn unstage_partial(&self, root: &Path, path: &Path, content: &[u8]) -> Result<(), String>;
}

// winc-editor/src/git/commit.rs
pub fn commit(&self, root: &Path, message: &str, amend: bool) -> Result<String, String>;
```

`content` 是工作区形式的目标 index 内容：调用方用 `winc_core::diff::pick_changes` 从当前 index 内容与磁盘内容组合得到。

## 3. 上游合并提示

合并 `upstream/dev` 时以下位置按 fork 的暂存模型解决，不要退回上游的内存勾选：

| 位置 | fork 取值 |
| --- | --- |
| `DiffTarget` | `StageHunk`／`StageLine`（上游 `Include`／`IncludeLine` 是旧模型） |
| `app/diff.rs` | `stage_diff_hunk`／`stage_diff_line`／`diff_commit_file` |
| `view/diff.rs` | `stage` 分支画 ＋／−，不画勾选框 |
| `view/git/commit.rs` | `stage_button` 画 ＋／−；浏览历史差异时（`commit.browse`）两者都不画、不记录命中 |
| `Git::stage` | `git.rs` 中的 `Result<String, String>`；`staging.rs` 不重复定义 |
| 格式化结果 | 保留 `job` 编号，`poll_formatting`／`formatting_done`／`wait_formatting` 三处一致 |

上游新增的 `CommitState::browse`／`with_log_list`／日志差异文件列表整体保留，只把行身份换成 `(组, 路径)`。

## 3. 契约

- **行身份**：`(仓库根目录, CommitGroup, 路径)`。同一路径可以同时出现在“已暂存的更改”和“更改”，禁止用 `Status::entries` 下标或单独路径区分。改名条目还带 `Entry::from`，整文件操作覆盖新旧两个路径。
- **分组**：合并更改（有冲突时）／已暂存的更改（`staged.is_some()`）／更改（`unstaged.is_some()`）。未跟踪未忽略文件在“更改”。
- **差异基准**：已暂存 = `HEAD` ↔ `index_content`，只读；更改 = `index_content` ↔ 当前文档或磁盘。`HEAD` 不变而 index 变化时也要刷新两侧。
- **提交**：`git commit --file=- --cleanup=whitespace`，修改上次提交时加 `--amend`。提交整个真实 index，不使用 `--only`，不在提交时把工作区内容加入 index。
- **路径**：“全部”指当前仓库，不是列表前 `STATUS_LIMIT` 项。交给 Git 的路径使用 `:(literal)` 前缀，重命名、删除与目录展开一并处理。
- **字节**：patch 比较 Git blob；展示文本经过滤器转换。`content_at`／`index_content` 返回检出形式，可与磁盘内容逐行比较。
- **保存**：暂存前保存所操作文件并等待在途保存与保存前格式化；保存失败、磁盘冲突或选择失效时停止，不改 index。
- **互斥**：暂存、提交、分支写操作互斥。占用按规范化工作树根目录记录，在后台写入结束时释放，不能依赖界面收取结果。
- **空暂存提交**：暂存区为空且有工作区改动时先确认“暂存全部并提交”（包含未跟踪且未忽略文件），顺序为保存 → 暂存全部 → 提交；取消不改动任何状态；提交失败保留 index 与提交信息。
- **文案**：`en-US` 与 `zh-CN` 成对增删；`winc-i18n` 的键一致与未使用消息测试必须保持通过。

## 4. 校验与错误

| 条件 | 结果 |
| --- | --- |
| 选择对应的内容已变化（保存、格式化、外部修改） | 返回选择已变化，刷新差异并提示重新选择；禁止回退为整文件暂存 |
| `index.lock` 被占用 | 返回 Git 的原因；index 不变 |
| 二进制、非 UTF-8、冲突条目 | 明确报错，部分操作不执行 |
| 符号链接 | 部分操作拒绝（`git-stage-partial-symlink`）；整文件操作可用 |
| 尚无首个提交 | `unstage` 用 `update-index --force-remove` 移除条目，文件回未跟踪 |
| 合并／拣选／还原进行中 | 部分操作不可用；提交使用当前 index |
| 提交被钩子或身份拒绝 | 保留 index 与提交信息，显示原因 |
| 项目关闭或换仓库时结果到达 | 按仓库根与请求编号丢弃，不写入当前项目 |

## 5. 情形

- 好：文件 v1 已提交 → 保存 v2 → 暂存 → 保存 v3；两组同时出现，提交得到 v2。
- 基准：只暂存一个文件的全部改动；其他已暂存文件保持原状。
- 坏：`git add` 委托给 shell 字符串拼接，路径里的 `[`、`*`、`?` 被当成通配符，连带暂存别的文件。

## 6. 必须的测试

- 提交只含 index：`a_commit_takes_the_index_not_the_worktree`。
- 部分暂存与取消：`partial_staging_writes_only_the_selected_hunks_and_lines`、`unstaging_one_hunk_keeps_the_other_one_staged`。
- 过期选择拒绝：`a_stale_selection_is_refused_instead_of_staging_the_file`。
- 首提交前取消暂存：`unstaging_before_the_first_commit_removes_the_entries`。
- 新增／删除／空文件／改名：`partial_staging_new_deleted_and_empty_files`、`staging_a_rename_covers_the_old_path_too`。
- 字节形态：`partial_staging_keeps_crlf_bom_and_missing_final_newline`。
- 特殊字符路径：`partial_staging_does_not_touch_files_with_similar_names`。
- 空暂存提交：`committing_an_unsaved_document_saves_it_before_staging`。
- 双组身份与唯一基准：`a_file_staged_in_the_terminal_appears_in_both_groups`。

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

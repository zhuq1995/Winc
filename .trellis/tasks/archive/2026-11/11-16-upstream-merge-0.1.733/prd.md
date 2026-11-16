# 同步上游 0.1.733

## Goal

把 `upstream/main` 合入 `dev`，在 fork 的三条自有化功能成立的前提下接收上游 0.1.733 的全部改动。

## 上游变更（检查时的状态）

| 项 | 值 |
| --- | --- |
| 未合并提交 | 104 |
| 变更规模 | 610 文件，+67572 / −34548 |
| VERSION | 0.1.660 → 0.1.733 |
| merge base | `5b8d3ea`（PR #181） |
| `upstream/main..upstream/dev` | 空，`main` 已含 `dev`（PR #228） |
| 冲突预演 | 49 个文件（`git merge-tree --write-tree dev upstream/main`） |

新增 crate：`winc-backend`、`winc-server`（远程开发）。新增依赖：`regex-syntax`、`x11-dl`。

上游重写了窗口生命周期：删除 `Shell`/`ShellState` 交接（`has_shell`、`take_shell`、`put_shell`、`wake_shell`、`take_shell_state`、`put_shell_state`、`needs_prompt_window`、`can_handoff_for_prompt`、`resume_pending_ui`），改为 `WindowKey`、`Recipient::{Window, Active}`、`Envelope`、`WindowProxy`、`Request::{NewWindow, Focus}`，窗口句柄由 `App.window.handle` 直接持有。

上游在 `crates/winc-core/src/diff/staging.rs` 提供了真实索引的部分暂存：`staged_picks`（第 22 行）、`stage`（第 48 行）、`unstage`（第 91 行），以及 `crates/winc-app/src/app/git/commit/staging.rs` 的 `stage_part`（第 69 行）。`Git::stage` 在两侧签名相同：`fn stage(&self, root: &Path, paths: &[PathBuf]) -> Result<String, String>`。

## Requirements

- **R1 完整合并**：上游 104 提交全部进入 `dev`，`git log dev..upstream/main` 为空。
- **R2 单窗口多项目**：合并完成后仍为所有项目共用一个系统窗口，最左侧项目列表切换当前项目。此条由子任务 `11-16-migrate-projects-window` 落地，本任务只负责不在合并中丢掉项目数据模型。
- **R3 暂存区用上游实现**：删除 fork 的 `crates/winc-editor/src/git/staging.rs` 与 `app/diff.rs::stage_diff_hunk`／`stage_diff_line`，改用上游 `winc_core::diff::{stage, unstage, staged_picks}`。界面仍保留 fork 的 ＋／− 按钮语义，不引入上游的 `Include`／`IncludeLine` 复选交互。
- **R4 格式化保存编号**：`app/editor/format.rs` 的 `Running.job`、`Format::next_id`、`poll_formatting`、`formatting_done`、`wait_formatting` 三处共用同一作业号，结果按（文档 id, 作业号）匹配。
- **R5 CI 保持删除**：`.github/workflows/ci.yml` 上游有改动、fork 已删除，合并结果为不存在该文件。
- **R6 构建与打包对齐**：`winc-backend`、`winc-server` 进入 workspace；发布流水线与 `AGENTS.md` 的打包说明随新 crate 更新。

## Constraints

- 只合 `upstream/main`。`upstream/dev` 已包含在内，不单独合。
- 按 `AGENTS.md`「上游合并流程」执行：试验分支 → 逐文件解冲突 → `cargo fmt --all` → `cargo clippy --workspace --all-targets` → `cargo test --workspace --no-fail-fast` → `--no-ff` 合回 `dev`。
- 禁止对 `dev` rebase 或 force push。
- 提交说明列出冲突取舍与触碰的上游高频文件（`winc.rs`、`app.rs`、`app/lifecycle/*`、`layout.rs`、`app/input/*`、`view/workspace/*`）。
- 已知基线失败：`winc-pty --test windows` 的伪控制台用例。

## Task Map

| 子任务 | 交付物 | 顺序 |
| --- | --- | --- |
| `11-16-merge-upstream-main` | 上游全量落地，Git 归并到上游实现，窗口生命周期接受上游版本 | 1 |
| `11-16-migrate-projects-window` | 项目列表迁入 `WindowKey`/`Request` 架构，恢复切换与关闭流程 | 2 |

两个子任务串行：子任务 2 在子任务 1 的合并结果上开工。

## Acceptance Criteria

- [ ] `git log --oneline dev..upstream/main` 无输出。
- [ ] `cargo fmt --all --check` 通过。
- [ ] `cargo clippy --workspace --all-targets` 无警告。
- [ ] `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast` 全绿（仅 `winc-pty --test windows` 伪控制台用例可失败）。
- [ ] 单窗口内新建、切换、关闭、拖拽排序项目的行为与合并前一致。
- [ ] 块级与行级部分暂存写入真实 git 索引，与终端里的 `git add`／`git reset` 共享状态。
- [ ] 保存触发的格式化按作业号丢弃过期结果。
- [ ] `.github/workflows/ci.yml` 不存在。
- [ ] `AGENTS.md` 的冲突取舍表与「自有化功能」一节反映合并后的实际实现。

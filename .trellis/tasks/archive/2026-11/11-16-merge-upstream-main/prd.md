# 合入 upstream/main 并归并 Git 实现

## Goal

在 `merge/upstream-main` 试验分支上把 `upstream/main` 合入，使仓库编译、测试通过，且 Git 暂存只保留上游一套实现。窗口生命周期接受上游版本，单窗口多项目由子任务 `11-16-migrate-projects-window` 在其结果上恢复。

## 前置状态

- `dev` 与 `upstream/main` 的 merge base 是 `5b8d3ea`。
- `git merge-tree --write-tree dev upstream/main` 报 49 个冲突文件。
- fork 的 projects 逻辑分布在 17 个文件（`app/snapshot/windows.rs` 28 处、`lifecycle/closing.rs` 15 处、`view/workspace/projects.rs` 13 处、`lifecycle/window.rs` 6 处、`lifecycle/startup.rs` 6 处、`app.rs` 6 处、`winc.rs` 4 处、`view.rs` 4 处、`layout.rs` 3 处、`input/mouse/regions.rs` 3 处、`input/mouse.rs` 3 处、`app/editor/format.rs` 3 处、`workspace/files.rs` 2 处，其余各 1 处）。

## Requirements

- **R1 Git 暂存归并到上游**：删除 `crates/winc-editor/src/git/staging.rs`；`app/diff.rs` 的 `stage_diff_hunk`／`stage_diff_line` 改接 `winc_core::diff::{stage, unstage, staged_picks}` 与 `app/git/commit/staging.rs::stage_part`。删除后不再有第二套索引文本计算。
- **R2 界面保留 ＋／− 暂存**：`view/diff.rs` 的 `DiffTarget::StageHunk`／`StageLine` 与行右缘的 ＋／− 按钮保留，取上游的 `Include`／`IncludeLine` 代码但不接入。
- **R3 窗口接受上游**：`crates/winc-app/src/app/lifecycle/window.rs`、`winc.rs`、`app.rs`、`app/lifecycle/*` 冲突处取上游实现；fork 的 `Shell` 交接与 projects 字段从这些文件移除。
- **R4 projects 代码移出**：`view/workspace/projects.rs` 与其在 `view.rs`、`layout.rs`、`app.rs`、`winc.rs` 的引用删除；`app/snapshot/windows.rs`、`lifecycle/closing/tests.rs`、`lifecycle/window/tests.rs` 的 projects 用例随上游版本移除。这些代码在 `5b8d3ea` 与 `c14f67e` 上可取回，供子任务 2 移植。
- **R5 格式化保存编号保留**：`app/editor/format.rs` 的作业号参数与匹配逻辑完整保留，`app/snapshot/editor/format.rs` 的对应用例保留。
- **R6 新 crate 纳入构建**：`winc-backend`、`winc-server` 编译通过；`Cargo.lock` 随上游更新。
- **R7 CI 保持删除**：`.github/workflows/ci.yml` 不恢复。

## Constraints

- 只合 `upstream/main`，不合 `upstream/dev`（已包含）。
- 合并期间不引入新功能、不重构上游代码；冲突取舍只做「取哪边」的判断。
- 上游 `feat:` 功能逐条核对：`git log --oneline <旧dev>..upstream/main` 中的每个特性提交在合并结果里有对应实现。
- 提交说明列出冲突取舍与触碰的高频文件：`winc.rs`、`app.rs`、`app/lifecycle/*`、`layout.rs`、`app/input/*`、`view/workspace/*`。
- 已知基线失败：`winc-pty --test windows` 的伪控制台用例。

## Acceptance Criteria

- [ ] `git log --oneline dev..upstream/main` 无输出。
- [ ] `cargo build --workspace` 通过。
- [ ] `cargo clippy --workspace --all-targets` 无警告。
- [ ] `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast` 全绿（仅 `winc-pty --test windows` 可失败）。
- [ ] `crates/winc-editor/src/git/staging.rs` 不存在；`rg "stage_partial" crates/` 无命中。
- [ ] `rg "staged_picks|winc_core::diff::stage" crates/winc-app/src/app` 命中 `app/git/commit/staging.rs` 与 `app/diff.rs`。
- [ ] `rg "projects_width|can_handoff_for_prompt|take_shell_state|deferred_close" crates/` 无命中（等待子任务 2 恢复的项目逻辑不在本任务范围）。
- [ ] `crates/winc-app/src/app/editor/format.rs` 保留作业号参数，`cargo test -p winc-app editor::format` 通过。
- [ ] `.github/workflows/ci.yml` 不存在。
- [ ] 合并提交的说明列出冲突取舍与触碰的高频文件。

## Out of Scope

- 单窗口项目列表的恢复（子任务 `11-16-migrate-projects-window`）。
- `AGENTS.md` 取舍表的最终定稿（父任务验收）。
- 打包脚本对新 crate 的适配（父任务验收项 R6 的一部分，实现放子任务 2 之后统一处理）。

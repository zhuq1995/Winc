# 合入上游 upstream/dev 更新

## Goal

把 `DEEIX-AI/Winc` 的 `upstream/dev`（`d79538b..5b8d3ea`，37 个非合并提交、49 个提交）合入 fork 的 `dev`，保留 fork 自有行为，跑通全量测试后推送 `origin/dev`。

## Requirements

- 按 `AGENTS.md`「上游合并流程」执行：`merge/upstream-dev` 试验分支 → `--no-ff --no-commit` 合并 → 逐文件解冲突 → `cargo fmt`／`clippy`／`test` → 合回 `dev` → 推送 `origin/dev` → 删除试验分支。
- `.github/workflows/ci.yml` 保持删除状态（fork 不跑上游 CI）。
- fork 不变量不得丢失：单窗口项目列表、真实暂存区（`DiffTarget::StageHunk`／`StageLine`）、`closing.confirming` 与 `deferred_close` 分离、格式化保存的 `job` 编号、每帧命中区域 `App::hits` + 自有 `projects_hits`／`projects_close`／`projects_keys`。
- 上游新增能力要落地到合并结果里，不能因冲突丢掉：Vim 编辑模式（新 `crates/winc-vim`）、diff 折叠与水平滚动条、搜索结果显示到预览标签、PDF 阅读位置、图片缩放、ArkTS 高亮、标题栏命中区域随缩放、Dock／任务栏最近文件夹。
- 上游高频文件（`winc.rs`、`app.rs`、`app/lifecycle/*`、`layout.rs`、`app/input/*`、`view/workspace/*`）的取值按 `AGENTS.md` 冲突取舍表处理，触碰点写进合并提交说明。

## Acceptance Criteria

- [ ] `git merge-base --is-ancestor upstream/dev dev` 成立（`upstream/dev` 全部提交已在 `dev` 上）。
- [ ] `cargo fmt --all --check` 无差异；`cargo clippy --workspace --all-targets` 无警告。
- [ ] `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast` 除已知基线（`winc-pty --test windows` 伪控制台用例）外全绿。
- [ ] `crates/winc-vim` 存在且被 workspace 收录，Vim 相关测试通过。
- [ ] fork 不变量逐条核对：项目列表切换仍单窗口、暂存区 ＋／− 仍可按块／行暂存、格式化保存编号一致、`.github/workflows/ci.yml` 仍为删除状态。
- [ ] 合并提交说明列出冲突取舍与触碰的上游高频文件；`dev` 已推送 `origin/dev`，试验分支已删除。

## Notes

- `.trellis/spec/backend/git-staging.md` 与 `window-projects.md` 记录 fork 不变量，解冲突时以它们为准。

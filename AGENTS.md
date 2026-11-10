<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->

<!-- FORK:START -->
## Fork 同步（upstream = DEEIX-AI/Winc）

本仓库是 `DEEIX-AI/Winc` 的 fork，自有功能与上游更新长期并存。以下规则适用于所有改动。

- `origin` 是自己的 fork，`upstream` 是原项目；只从 `upstream/main` 拉更新。
- 同步：`git fetch upstream && git merge upstream/main`。禁止对 `dev` rebase 或 force push。
- 合并前先提交本地改动，保持工作区干净；冲突只在 `dev`（或功能分支）上解决一次。
- fork 自有代码放在新文件、新模块里；上游文件优先加调用点，不改写函数体。禁止全仓格式化、批量改名、移动上游模块——这些会让之后每次合并都冲突。
- 上游高频文件：`crates/winc-app/src/winc.rs`、`app.rs`、`app/lifecycle/*`、`layout.rs`、`app/input/*`、`view/workspace/*`。改这些文件时在提交说明里写清触碰的上游函数，便于合并时定位。
- 合并后运行 `cargo test -p winc-app`。项目切换、终端连续、退出确认是本 fork 的不变量，编译通过不代表行为正确。
- 窗口、事件路由与项目状态的约定见 `.trellis/spec/backend/window-projects.md`，改动前先读。
- 换行统一 LF：`git config core.autocrlf false`，与 CI 和发布脚本一致；否则快照测试会因 CRLF 失败。
- 上游重写窗口生命周期时，`Shell` 交接需要手工重新推导；到那一步改为在 upstream tag 上维护补丁集，或把功能推回上游，不再整仓合并。
<!-- FORK:END -->

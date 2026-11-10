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

- `origin` 是自己的 fork，`upstream` 是原项目；常规更新只从 `upstream/main` 拉。
- `upstream/dev` 的提交已合入 `dev`：从 `dev` 新建 `merge/upstream-dev`，合并、解决冲突、跑测试后 `--no-ff` 合回 `dev`，再删试验分支。
- 同步：`git fetch upstream && git merge upstream/main`。禁止对 `dev` rebase 或 force push。
- 分支布局：`mirror` 只跟 `upstream/main`，是只读基线，不在上面提交；`dev` 是集成线，只接收上游合并与功能分支；私有开发在 `feat/*` 上进行，完成后 `git merge --no-ff` 回 `dev`。
- `feat/*` 生命周期控制在一周内；长期不回流的私有分支等于第二条分叉，冲突成本随上游提交线性增加。
- Trellis 工具文件（`.trellis/`、`.agents/`、`.pi/`）只在 `chore/trellis-tooling` 分支上做版本管理，永远不合入 `dev`。该分支检出在 `../Winc-trellis` worktree（稀疏检出，只含工具路径）；主线工作区的 `.trellis`、`.agents`、`.pi` 是指向它的 junction。
- 工具配置、任务、journal 的改动在 `../Winc-trellis` 里 `git add` 并提交、推送到 `origin/chore/trellis-tooling`；不要在主线工作区提交这些路径。
- 主线用 per-worktree 的排除文件忽略三个工具目录：`git config extensions.worktreeConfig true`，再把 `core.excludesFile` 指到 `.git/winc-excludes`（只在主线 worktree 生效）。`.git/info/exclude` 由所有 worktree 共用，写在那里会让工具 worktree 也无法跟踪 `.trellis/`。提交代码用显式路径（`git add crates AGENTS.md`），不要用 `git add -A`。
- junction 只在 Windows 上有效。换机器或重新 clone 后按上面步骤重建 worktree 和目录联接，`git worktree list` 可确认现状。
- 合并前先提交本地改动，保持工作区干净；冲突只在 `dev`（或功能分支）上解决一次。
- fork 自有代码放在新文件、新模块里；上游文件优先加调用点，不改写函数体。禁止全仓格式化、批量改名、移动上游模块——这些会让之后每次合并都冲突。
- 上游高频文件：`crates/winc-app/src/winc.rs`、`app.rs`、`app/lifecycle/*`、`layout.rs`、`app/input/*`、`view/workspace/*`。改这些文件时在提交说明里写清触碰的上游函数，便于合并时定位。
- 合并后运行 `cargo test -p winc-app`。项目切换、终端连续、退出确认是本 fork 的不变量，编译通过不代表行为正确。
- 窗口、事件路由与项目状态的约定见 `.trellis/spec/backend/window-projects.md`，改动前先读。
- 换行统一 LF：`git config core.autocrlf false`，与 CI 和发布脚本一致；否则快照测试会因 CRLF 失败。
- 上游重写窗口生命周期时，`Shell` 交接需要手工重新推导；到那一步改为在 upstream tag 上维护补丁集，或把功能推回上游，不再整仓合并。
<!-- FORK:END -->

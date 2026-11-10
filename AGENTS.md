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
- `upstream/dev` 的提交已合入 `dev`；后续的上游 `dev` 更新按下文「上游合并流程」执行。
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

### 上游合并流程

`upstream/main`：在 `dev` 上直接 `git fetch upstream && git merge upstream/main`。

`upstream/dev`：不直接合入 `dev`，按以下顺序走。

1. `git switch -c merge/upstream-dev dev`，`git merge --no-ff --no-commit upstream/dev`。
2. 逐文件解决冲突，取舍见下表；每解决一个 `git add <文件>`，`git diff --name-only --diff-filter=U` 为空后继续。
3. `cargo fmt --all`；`cargo clippy --workspace --all-targets` 无警告。
4. `PATH="/c/Program Files/Git/cmd:$PATH" cargo test -p winc-app -p winc-editor -p winc-i18n --no-fail-fast` 全绿。
5. 在试验分支提交合并，提交说明列出冲突取舍与触碰的上游高频文件；`git switch dev && git merge --no-ff merge/upstream-dev`，推送 `origin/dev`，删除试验分支。
6. 合并带来的规范变化写进 `.trellis/spec/backend/*`（在 `../Winc-trellis` 提交推送）；本节的取舍表随之下沉。

合并提交前确认上游新功能没有被冲突解决丢掉：逐个检查 `git log --oneline <旧dev>..upstream/dev` 中的 `feat:` 提交在合并结果里有对应实现与测试。

冲突取舍：

| 上游改动 | fork 取值 |
| --- | --- |
| 提交列表勾选框（`Include`／`IncludeLine`、`toggle_diff_*`、`reset_checks`） | fork 的 ＋／− 暂存：`DiffTarget::StageHunk`／`StageLine`、`stage_diff_*`、`commit.staging` 互斥 |
| `Git::stage` 签名 | 只保留 `git.rs` 中的 `Result<String, String>`，`staging.rs` 不重复定义；调用点补 `.map(|_| ())` |
| 提交视图行身份 | `(CommitGroup, 路径)`；上游的 `browse`／`with_log_list` 保留，浏览历史差异时隐藏暂存控件并跳过其命中区域 |
| 格式化结果 | 保留 fork 的 `job` 编号，`poll_formatting`／`formatting_done`／`wait_formatting` 三处一致 |
| 项目栏与窗口生命周期 | fork 的单窗口逻辑优先（`layout.projects`、`traffic_lights_moved`、`Target::Blocked` 等），采纳上游新增分支与文案 |

测试注意：Git 相关测试需要 `PATH` 含 Git 的 `cmd` 目录（`sh` 由它提供），否则本地远程测试失败。
<!-- FORK:END -->

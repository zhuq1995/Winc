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
- fork 不跑 GitHub Actions 的 CI：`.github/workflows/ci.yml` 已删除（每次推 `dev` 都在 macOS clippy、Windows 测试上失败并寄邮件）。上游改动该文件时保持删除，构建检查用本地命令。
- 上游重写窗口生命周期时，`Shell` 交接需要手工重新推导；到那一步改为在 upstream tag 上维护补丁集，或把功能推回上游，不再整仓合并。

### 自有化功能

合并时先对照本节：每条都是 fork 行为，上游的同类实现不能静默覆盖它。

#### 单窗口多项目

- 行为：所有项目共用一个系统窗口，最左侧的项目列表切换当前项目；每个项目保留自己的标签、文档、终端连接和后台任务。项目数大于 1 时列表宽度为 `PROJECTS_WIDTH`，否则为 0（`App::projects_width`）。多个目录参数（含 worktree）各开一个项目；`--new-instance`、`--wait`、标准输入保持独立的进程与窗口。
- 代码：`view/workspace/projects.rs`（自有文件）、`layout.rs` 的 `Layout::projects`、`app/lifecycle/window.rs` 的 `projects_width`、`place_traffic_lights`、`winc.rs` 的 `Project`、窗口交接、`app/lifecycle/closing.rs` 的 `request_close_project`、`defer_close`、`app.rs` 的 `projects_hits`／`projects_close`／`projects_keys`／`projects_scroll`／`projects_reveal`。
- 不变量：

  | 场景 | 要求 |
  | --- | --- |
  | 切换项目 | 保留窗口句柄、渲染器、主题、修饰键和焦点，只换项目数据 |
  | 交接给待处理提示 | 有模态界面（提示、另存为、终端确认、macOS sheet）的项目不接收窗口，见 `can_handoff_for_prompt` |
  | 保存或格式化进行中的关闭 | 请求并入 `deferred_close`，退出优先于关闭项目 |
  | 退出进行中 | 不再新建项目 |
  | 关闭项目 | 复用已有的保存、备份、终端确认流程；最后一个项目停在开始页 |
  | macOS 关闭窗口 | 只隐藏窗口，其他平台照常退出 |

- 测试：`app/snapshot/windows.rs`、`app/lifecycle/closing/tests.rs`、`app/lifecycle/events/tests.rs`、`app/lifecycle/window/tests.rs`、`app/snapshot/terminal/editor.rs`、`winc/tests.rs`。

#### 真实暂存区

- 行为：提交面板分成已暂存和未暂存两组，暂存与取消暂存写真实 Git 索引，与终端里的 `git add`／`git reset` 共享状态；提交只取暂存区内容。块级和行级的部分暂存生成只含目标改动的统一差异，交给 `git apply --cached`；索引为空时提交先提示全部暂存。
- 代码：`winc-editor/src/git/staging.rs`（自有文件）、`app/git/commit/stage.rs`（自有文件）、`app/diff.rs` 的 `stage_diff_hunk`／`stage_diff_line`、`view/diff.rs` 的 `DiffTarget::StageHunk`／`StageLine` 命中区域。
- 不变量：

  | 场景 | 要求 |
  | --- | --- |
  | 暂存与提交 | 互斥，由 `commit.staging` 表示 |
  | 提交视图的行身份 | `(CommitGroup, 路径)`；浏览历史差异时隐藏暂存控件并跳过其命中区域 |
  | 部分暂存的目标 | 符号链接只提供整文件暂存（`git-stage-partial-symlink`），不按链接目标的内容做部分暂存 |
  | 暂存写入期间关闭项目 | 写完释放 worktree 占用，不泄漏 |
  | 上游同名的 `Git::stage` | 只保留 `git.rs` 中返回 `Result<String, String>` 的那个，`staging.rs` 不重复定义，调用点补 `.map(|_| ())` |

- 测试：`app/snapshot/git/commit.rs`、`app/snapshot/git/diff.rs`、`app/git/changes/tests.rs`、`winc-editor/src/git/tests.rs`。

#### 格式化保存编号

- 行为：保存触发的格式化作业带编号，结果按（文档 id, 作业号）匹配，过期的结果直接丢弃，不写回已改动的文档。
- 代码：`app/editor/format.rs` 的 `Running.job`、`Format::next_id`、`poll_formatting`、`formatting_done`、`wait_formatting`。
- 不变量：三处使用同一个作业号；合并上游格式化改动时保留编号参数，不要退回按文档 id 单独匹配。
- 测试：`app/editor/format/tests.rs`、`app/snapshot/editor/format.rs`。

### 上游合并流程

`upstream/main`：在 `dev` 上直接 `git fetch upstream && git merge upstream/main`。

`upstream/dev`：不直接合入 `dev`，按以下顺序走。

1. `git switch -c merge/upstream-dev dev`，`git merge --no-ff --no-commit upstream/dev`。
2. 逐文件解决冲突，取舍见下表；每解决一个 `git add <文件>`，`git diff --name-only --diff-filter=U` 为空后继续。
3. `cargo fmt --all`；`cargo clippy --workspace --all-targets` 无警告。
4. `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast` 全绿；已知基线失败只有 `winc-pty --test windows` 的伪控制台用例。
5. 在试验分支提交合并，提交说明列出冲突取舍与触碰的上游高频文件；`git switch dev && git merge --no-ff merge/upstream-dev`，推送 `origin/dev`，删除试验分支。
6. 合并带来的规范变化写进 `.trellis/spec/backend/*`（在 `../Winc-trellis` 提交推送）；本节的取舍表随之下沉。

合并提交前确认上游新功能没有被冲突解决丢掉：逐个检查 `git log --oneline <旧dev>..upstream/dev` 中的 `feat:` 提交在合并结果里有对应实现与测试。

合并的总原则：冲突处以自有化功能为主，不因上游改动丢掉单窗口项目列表、真实暂存区、格式化保存编号等行为。上游提供了同一功能时反过来——用上游实现替换自有实现，删掉 fork 版本，避免两份代码并存。

冲突取舍：

| 上游改动 | fork 取值 |
| --- | --- |
| 提交列表勾选框（`Include`／`IncludeLine`、`toggle_diff_*`、`reset_checks`） | fork 的 ＋／− 暂存：`DiffTarget::StageHunk`／`StageLine`、`stage_diff_*`、`commit.staging` 互斥 |
| `Git::stage` 签名 | 只保留 `git.rs` 中的 `Result<String, String>`，`staging.rs` 不重复定义；调用点补 `.map(|_| ())` |
| 每帧命中区域 | 采纳上游 `App::hits`（`hits.commit`、`hits.tree_tools` 等）；项目列表的 `projects_hits`／`projects_close`／`projects_keys` 是自有字段，留在 `App` 上 |
| 字段分组 | 采纳上游 `tree`／`sidebar`／`picker`／`find`／`closing`；`deferred_close`（推迟的关闭请求）与 `closing.confirming`（终端确认）是两个概念，保持分开 |
| 暂存、提交视图行身份、格式化作业号 | 见上文「自有化功能」，按其中的不变量取舍 |
| 项目栏与窗口生命周期 | fork 的单窗口逻辑优先（`layout.projects`、`traffic_lights_moved`、`Target::Blocked` 等），采纳上游新增分支与文案 |

测试注意：Git 相关测试需要 `PATH` 含 Git 的 `cmd` 目录（`sh` 由它提供），否则本地远程测试失败。
<!-- FORK:END -->

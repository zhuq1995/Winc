# Winc fork 功能保留清单

本清单以应用代码 `d54201a`、上游 `v0.1.733` 为定位基线。行为契约在实现改名后仍适用；代码位置随合并更新。下文路径均相对于应用仓库根目录。

## 目录

- [功能索引](#功能索引)
- [项目与窗口 F01–F08](#项目与窗口-f01f08)
- [Git 与菜单 F09–F14](#git-与菜单-f09f14)
- [格式化与平台 F15–F17](#格式化与平台-f15f17)
- [仓库与发布 F18–F20](#仓库与发布-f18f20)
- [当前契约复核项](#当前契约复核项)
- [验收方法](#验收方法)

## 功能索引

| ID | 保留的行为 | 定位起点 |
| --- | --- | --- |
| F01 | 单窗口承载多个独立项目，成对交接窗口与渲染器 | `crates/winc-app/src/winc.rs` |
| F02 | 最左侧项目列表、宽度条件、滚动与命中隔离 | `crates/winc-app/src/view/workspace/projects.rs` |
| F03 | 项目拖拽排序与当前项目保持 | `crates/winc-app/src/app/input/mouse/drag.rs` |
| F04 | 文件夹名称、同名目录父级区分、开始页与忙碌标记 | `crates/winc-app/src/winc.rs` |
| F05 | 多目录参数、worktree 与外部打开路由到项目 | `crates/winc-app/src/lib.rs` |
| F06 | 单个项目关闭、最后一个项目回开始页 | `crates/winc-app/src/app/lifecycle/closing.rs` |
| F07 | 提示归属、保存期间推迟关闭、退出优先与禁止新建 | `crates/winc-app/src/app/lifecycle/closing.rs` |
| F08 | 项目现场、备份恢复、独立进程与真正空项目复用 | `crates/winc-app/src/app/workspace/session.rs` |
| F09 | UI 与命令行共享真实 Git 索引，提交取索引 | `crates/winc-app/src/app/git/commit/staging.rs` |
| F10 | 暂存区默认开启、两组视图、行尾 ＋／− 与设置开关 | `crates/winc-app/src/app/settings.rs` |
| F11 | 块与行暂存、历史差异隔离、路径与符号链接边界 | `crates/winc-core/src/diff/staging.rs` |
| F12 | 暂存队列、提交互斥、根目录与 worktree 生命周期 | `crates/winc-app/src/app/git/commit/staging.rs` |
| F13 | 分支菜单复制分支名，含当前分支 | `crates/winc-app/src/app/git/branch.rs` |
| F14 | 位置菜单复制工作树名 | `crates/winc-app/src/app/input/context_menu/run.rs` |
| F15 | 格式化结果与保存审批按作业号匹配 | `crates/winc-app/src/app/editor/format.rs` |
| F16 | macOS 窗口句柄与隐藏行为、终端焦点和原生显示资源 | `crates/winc-app/src/app/lifecycle/window.rs` |
| F17 | LF 测试仓库、Windows Git 静默运行与确定性等待 | `crates/winc-editor/src/test_util.rs` |
| F18 | fork 的 CI 文件保持删除，发布工作流单独保留 | `.github/workflows/ci.yml` |
| F19 | 分支边界、工具文件随当前分支管理 | [维护规则](fork-maintenance.md) |
| F20 | 本地 Windows NSIS 与便携包完整性 | `release/windows/package.ps1` |

## 项目与窗口 F01–F08

### F01 单窗口与窗口交接

- 契约：同一普通实例的项目共用一个系统窗口；每个项目保留文档、标签、编辑状态、终端连接与后台任务。
- 实现：`crates/winc-app/src/winc.rs` 的 `Slot`、`Project`、`Shared::projects`、`Winc::activate`；`crates/winc-app/src/app/lifecycle/window.rs` 的 `has_window`、`take_window`、`put_window`、`take_window_state`、`put_window_state`；`crates/winc-app/src/app.rs` 的项目字段。
- 交接内容：窗口句柄与 `FrameState.renderer` 成对移动，显示链接和 macOS traffic lights 随窗口移动；保留 frame、controls、主题、修饰键、鼠标及焦点，清理拖动、悬停、提示、IME 预编辑与终端旧焦点。
- 新项目的 `App` 通过 `activate` 接收窗口；`App::resumed` 只对当前项目创建窗口。检查所有切换入口是否调用交接，而非仅改 `active`。
- 检查：A→B→A 后窗口句柄与 renderer 身份相同，两个项目的标签/光标独立，终端进程持续输出；离屏请求测试只覆盖切换请求。

### F02 项目列表布局与输入

- 契约：项目数大于 1 时左侧列表宽 `PROJECTS_WIDTH = 190.0`，否则为 0；列表与侧栏、标题栏控件及主区分隔。
- 实现：`crates/winc-app/src/layout.rs` 的 `Layout::projects`、`PROJECT_ROW = 30.0`、`PROJECT_GAP = 2.0`、`PROJECT_PAD = 6.0`、`projects_max_scroll`；`crates/winc-app/src/theme.rs`；`crates/winc-app/src/view/workspace/projects.rs`。
- 输入：`crates/winc-app/src/app/input/mouse/regions.rs::project_at` 校验 `projects_keys`，关闭按钮优先；`crates/winc-app/src/app/input/mouse/target.rs` 用 `Target::Blocked` 隔离主区；`crates/winc-app/src/app/scroll/wheel.rs::scroll_projects` 限制滚动并清除旧命中区域。
- 检查：1/2/很多项目三种数量；列表滚动后命中正确；picker 打开、侧栏滑动和窄窗口时不穿透；当前项目滚动可见。
- 测试：`crates/winc-app/src/app/snapshot/projects.rs` 的宽度、条目测试；`crates/winc-app/src/layout/tests.rs`。

### F03 拖拽排序

- 入口：左键按住项目条目上下拖动；普通点击切换。
- 实现：`crates/winc-app/src/app.rs::Drag::Project`；`crates/winc-app/src/app/input/mouse/drag.rs` 的阈值 4.0；`crates/winc-app/src/app/input/mouse.rs` 在松开时把间隙编号换成最终下标；`crates/winc-app/src/winc.rs` 的 `Request::MoveProject` 与 `move_project`。
- 契约：移动第一项/最后一项、上下两个方向均正确，当前项目与终端保留；原位松开不重排，Escape 取消拖动。
- 检查：`crates/winc-app/src/app/snapshot/projects.rs::dragging_a_project_moves_it`；同时检查实际拖动、滚动后的落点与退出再恢复的顺序。

### F04 项目名称与标记

- 实现：`crates/winc-app/src/winc.rs` 的 `project_label(root: &Path) -> (String, String)` 与 `refresh_shared`；`crates/winc-app/src/view/workspace/projects.rs` 绘制徽标、名称、忙碌圆点与悬停关闭按钮。
- 契约：使用文件夹名，同名目录显示 `parent/name`；无目录显示开始页；长名称截断、当前项高亮。忙碌项未悬停时显示圆点，关闭按钮在悬停时显示。
- 文案：`crates/winc-i18n/locales/en-US/workspace.ftl` 与 `crates/winc-i18n/locales/zh-CN/workspace.ftl` 的 `workspace-projects-start-page`。
- 测试：`crates/winc-app/src/winc/tests.rs::project_names`；`crates/winc-app/src/app/snapshot/projects.rs::projects_with_the_same_folder_name_show_their_parent`。

### F05 打开目录与 worktree

- 契约：多个目录参数各开一个项目；worktree 选择器打开新位置时保留现有项目；再次打开已有目录激活其项目；外部进程与文件管理器交来的路径遵守相同路由。
- 实现：`crates/winc-app/src/lib.rs::open_targets` 返回额外项目目录；`crates/winc-app/src/winc.rs` 的 `open_projects`、`restore_windows`、`open_targets`、`window_for`、`new_window`、`launched`、`opened`；`crates/winc-app/src/app/git/location_picker.rs` 的 `open_folder(&place.path)`；`crates/winc-app/src/app/git/worktree.rs::open_worktree`。
- 检查：启动参数两个目录、选择另一个 worktree、打开后台项目内的文件、再次打开后台目录；每种入口都确认窗口实际交接及已开标签保留。
- 测试：`crates/winc-app/src/app/snapshot/workspace/session.rs::extra_folders_on_the_command_line_open_as_separate_projects` 只覆盖启动参数拆分；其余入口需补行为证据。

### F06 项目关闭

- 入口：项目条目右侧 ✕；关闭项目命令与右键入口按当前实现检查。
- 实现：`crates/winc-app/src/winc.rs` 的 `Request::CloseProject`、`close_project`、`settle_projects`；`crates/winc-app/src/app/lifecycle/closing.rs` 的 `CloseRequest::CloseProject`、`request_close_project`、`finish_close`；`crates/winc-app/src/app/terminal.rs` 的终端确认。
- 契约：只关闭目标项目；保留其他项目，沿用保存与终端确认；关闭当前项后选择相邻项目，最后一个项目保留窗口并停在开始页。关闭项目与系统关闭窗口分别验收。
- 测试：`crates/winc-app/src/app/snapshot/projects.rs` 的两个关闭测试。命令与右键的当前缺口见后文。

### F07 提示归属、保存与退出

- 契约：提示由所属项目承载，窗口同一时刻归一个项目；原生另存为、macOS sheet、终端确认、格式化审批与 worktree 删除确认都检查项目归属。
- 实现：`crates/winc-app/src/app/lifecycle/closing.rs` 的 `deferred_close`、`defer_close`、`resume_deferred_close`、`closing_pending`；`crates/winc-app/src/app/editor/format.rs::save_with_notes` 完成保存后继续关闭，保存失败清除推迟的关闭。
- 契约：保存/格式化期间关闭项目只排队一次，退出优先于关闭项目；取消或失败后保留文档与项目；退出进行中阻止全部新建项目入口。
- 检查：慢格式化期间关项目→完成后关闭；保存失败与取消；提示期间切换项目；退出期间命令面板/菜单/外部路径请求新建。当前源码覆盖不完整，逐项记录结果。

### F08 现场、备份与独立进程

- 契约：项目状态与备份独立；重启恢复目录和标签；空项目的未命名内容有恢复途径；复用开始页前确认文档、终端与待办操作都为空。
- 实现：`crates/winc-app/src/winc.rs` 的 `remember_windows`、`restore_windows`、`blank_window`；`crates/winc-app/src/app/workspace/session.rs`；`crates/winc-app/src/lib.rs` 与 `crates/winc-app/src/main.rs` 的启动/one-off 路径。
- `--new-instance`、`--wait`、标准输入使用独立实例语义；one-off 不污染普通项目的最近列表、恢复与备份。
- 测试入口：`crates/winc-app/src/app/snapshot/workspace/session.rs`、`crates/winc-app/src/app/lifecycle/one_off/tests.rs`；先确认相应模块与平台条件被加载。

## Git 与菜单 F09–F14

### F09 真实索引

- 契约：暂存、取消暂存读写 Git 索引，与终端 `git add`／`git reset` 同步；暂存区视图的提交仅取索引内容；空索引提交前提示全部暂存。当前空索引路径与该提示契约不同，见复核表。
- 实现：`crates/winc-app/src/app/git/commit/staging.rs` 的 `stage_paths`、`stage_part`、`write_part`、`sync_index_texts`；`crates/winc-editor/src/git.rs` 的 `Git::stage`、`Git::unstage`、`Git::stage_content`；`crates/winc-app/src/app/git/commit/state.rs` 的索引内容与状态。
- 当前部分暂存计算三份文本，调用 `stage_content` 写入索引内容；合并按行为验收，不要求固定为 `git apply --cached`。
- 检查：文件产生两处改动，只暂存一处，比较 `git diff` 与 `git diff --cached`，提交后另一处仍在工作区；终端改索引后 UI 刷新一致。

### F10 默认暂存区与按钮

- 默认：`git.stagingArea` 缺省即开启，显式 `false` 关闭。以下四处都是 `git_staging_area != Some(false)`：
  - `crates/winc-app/src/lib.rs`：首次启动。
  - `crates/winc-app/src/winc.rs::new_window`：新项目初始化。
  - `crates/winc-app/src/app/git/commit/staging.rs::apply_staging_view`：应用设置。
  - `crates/winc-app/src/app/settings.rs`：设置选中状态。
- `crates/winc-config/src/config.rs::Config::git_staging_area` 的注释与四处一致。
- 入口：设置 › Git › 显示暂存区；提交视图 ⋯ 菜单的 `CommitStagingArea`／`CommitChangeList`；`toggle_staging_area`。
- UI：暂存区视图按已暂存/未暂存分组，文件行尾 `CommitTarget::StageFile(index, on)` 显示 ＋／−；`crates/winc-app/src/view/git/commit.rs::file_action_targets` 负责命中。差异块/行采用上游 `DiffTarget::Include`／`IncludeLine` 复选框。
- 目录行尾 `CommitTarget::DirStage(row, on)` 与分组标题 `GroupStage(group)` 同样提供 ＋／−；目录范围来自 `commit_rows().dir(group, dir)`，包含折叠的子目录且只取当前组内文件。长目录名展开时按钮随全名移动，包含顶部固定目录；名称裁剪、提示与点击命中保持一致；历史浏览、勾选式和冲突目录隐藏暂存按钮。
- 目录回归入口：`crates/winc-app/src/app/snapshot/directory_staging.rs` 使用真实 Git 索引和离屏鼠标操作；检查目录外已暂存文件、其他分组未跟踪文件及磁盘内容保持原状，并检查长名展开与固定目录按钮的命中。
- 命令：`GitStageAll`、`GitUnstageAll`、`GitStageFile`；`stage_all` 暂存已跟踪改动，未跟踪文件逐个加入。
- 检查：全新配置首次启动、第二个项目、切换设置、重启四条路径；显式 false 仍可切到勾选式。只保留核心索引代码不能证明默认视图或入口保留。

### F11 部分暂存与边界

- 实现：`crates/winc-core/src/diff/staging.rs` 的 `stage`、`unstage`、`staged_picks`；`crates/winc-app/src/app/diff.rs::toggle_diff_part`；`crates/winc-app/src/app/git/commit/state.rs::stages_by_hunk`；`crates/winc-app/src/view/diff.rs`。
- 契约：整文件、整块、单行暂存/取消暂存正确；提交视图用 `(CommitGroup, 路径)` 区分同名两组；历史差异隐藏暂存控件并移除命中区域。
- 契约：符号链接只整文件暂存，不读取链接目标做部分暂存；二进制/非 UTF-8 的边界有明确反馈；路径含通配符按字面文件名处理，合并冲突标记未清除时不误标已解决。
- 保留上游唯一实现，禁止恢复 `crates/winc-editor/src/git/staging.rs`、`crates/winc-app/src/app/git/commit/stage.rs` 两份并行实现。
- 检查：核心纯函数测试与真实仓库测试都跑；链接、重命名、未跟踪文件、索引与磁盘都变化、历史浏览分别检查。符号链接契约的当前覆盖见后文。

### F12 暂存/提交队列与资源

- 实现：`crates/winc-app/src/app/git/commit/staging.rs` 的 `StageJob::{Add,Remove,Part}`、`push_staging`、`pump_staging`、`staging_finished`；`crates/winc-app/src/app/git/commit.rs` 的 `staging_running`、`staging_queue`、`commit_queued` 与 `staging_active`。
- 契约：索引写入串行；提交等待写入结束；失败清队列并按真实状态刷新；同一 worktree 的写入由共享占用协调，关闭项目、切仓库与删除 worktree 时后台操作最终释放资源。
- `CommitState::staging()` 表示视图开关，不能作为写入互斥标志；`staging_finished` 根据 root 匹配结果归属。
- 检查：连续点暂存后提交、索引锁失败、写入中切项目/关闭项目、操作完移除 worktree；检查内容与锁文件，不能只看 busy 图标。

### F13 复制分支名

- 实现：`crates/winc-app/src/app/git/branch.rs` 的 `BranchOp::CopyName`、`GROUPS`、当前分支过滤、`label`、`run_branch_op`。
- 契约：分支操作菜单有「复制分支名」，含当前、本地与远程分支；Git 忙碌时复制入口仍可用；复制真实名称到剪贴板，无换行且不执行切分支。
- 文案：`crates/winc-i18n/locales/en-US/git.ftl` 与 `crates/winc-i18n/locales/zh-CN/git.ftl` 的 `git-branch-copy-name`。
- 测试：`crates/winc-app/src/app/git/branch/tests.rs::branch_ops_are_grouped_with_separators` 检查菜单分组，另检查菜单可用性与剪贴板内容。

### F14 复制工作树名

- 实现：`crates/winc-app/src/app/input/context_menu.rs::MenuCommand::LocationCopyName`；`crates/winc-app/src/app/input/context_menu/present.rs` 的位置菜单；`crates/winc-app/src/app/input/context_menu/run.rs` 调用 `winc_editor::git::repo_name(&path)` 后 `set_clipboard(name, false)`。
- 契约：位置/worktree 菜单有「复制工作树名」，复制目标工作树名；保持与复制分支名、复制路径不同的入口。
- 文案：`crates/winc-i18n/locales/en-US/git.ftl` 与 `crates/winc-i18n/locales/zh-CN/git.ftl` 的 `git-location-copy-name`。
- 检查：普通仓库与 linked worktree，选中不同项后复制；双语菜单键与参数一致。

## 格式化与平台 F15–F17

### F15 格式化保存作业号

- 实现：`crates/winc-app/src/app/editor/format.rs` 的 `FormatState::next_id`、`Running.job`、`Outcome = (DocumentId, u64, Result<Formatted, FormatError>)`、`launch`、`poll_formatting`、`formatting_done`、`ToolAnswer.save_id`、`answer_tool_choice`、`saving`、`save_with_notes`。
- 契约：结果按文档 ID 与 job 匹配；同文档新作业不会被旧结果覆盖；保存审批按 save_id 匹配；取消清除对应记录，失败保留文档并清除推迟关闭。
- 测试：`crates/winc-app/src/app/editor/format/tests.rs::a_stale_format_result_is_discarded`；`crates/winc-app/src/app/snapshot/editor/format.rs`。当前模块为 Unix 条件，Windows 的测试通过不包含它们。
- 检查：旧结果、新结果交错；旧审批晚于新保存；格式化失败、保存失败、保存中关项目；编号匹配与文档内容版本检查分别核对。

### F16 平台窗口与终端焦点

- 契约：macOS 的窗口句柄从 `App.window.handle` 获取；traffic lights、显示链接、IME 和主题随实际窗口交接；窗口切走时解除终端前台焦点，终端连接保留。
- 平台关闭契约：macOS 系统关闭按钮隐藏窗口、保留应用；其他平台普通窗口关闭执行退出流程。多项目条件下检查此契约与上游 `shared.others()` 分支的关系。
- 实现：`crates/winc-app/src/app/lifecycle/window.rs`、`crates/winc-app/src/app/lifecycle/events.rs`、`crates/winc-app/src/app/lifecycle/closing.rs`、`crates/winc-app/src/app/terminal.rs`。
- 检查：Windows 运行测试、Linux 分支编译和 Unix 测试、macOS 运行或目标编译各记录环境。现存旧 API 测试文件是否编译见后文。

### F17 可重复的本地测试

- 契约：测试临时仓库不受主机 CRLF 设置影响；Git 子进程在 Windows 上无额外控制台窗口；异步测试按状态或通道完成等待。
- 实现：`crates/winc-editor/src/test_util.rs::quiet` 用 Windows `creation_flags(0x0800_0000)`；`crates/winc-editor/src/git/rebase/tests.rs`、`crates/winc-editor/src/git/sync/tests.rs`、`crates/winc-editor/src/git/worktree/tests.rs` 调用静默包装。sync fixture 保留 `core.autocrlf=false`；其他 fixture 分别核对。
- 检查：确认 Git `cmd` 在测试 PATH 中；进程级 Git 配置与 fixture 设置优先；机器全局配置保持原值。存在 Git 环境注入时合并它的配置项，避免覆盖调用者设置。
- 契约来源还包括确定性等待；检查异步完成条件，避免用机器速度或固定毫秒阈值作为正确性标准。

## 仓库与发布 F18–F20

### F18 CI 边界

- `.github/workflows/ci.yml` 在 fork 中保持删除，上游修改该文件时维持删除状态。
- `.github/workflows/release-windows.yml` 是发布流程，按自身授权与需求保留；CI 删除不扩展成删除全部工作流。
- 检查：应用结果树、暂存区与发布文件分别核对；构建与回归在本地完成。

### F19 分支与工具文件版本管理

- `origin` 为 fork，`upstream` 为 DEEIX-AI/Winc；`mirror` 是只读上游 main 基线；`dev` 接收合并；私有开发在 `feat/*`，完成后 `--no-ff` 回 dev。
- 禁止对 dev rebase 或 force push。合并前工作区干净，使用显式路径暂存，保持 LF。
- `.trellis/`、`.agents/`、`.pi/` 为主工作区普通目录，在当前开发分支直接跟踪；技能、规范、任务和 journal 随项目提交。
- 本机身份、任务指针、运行状态和缓存按 `.trellis/.gitignore` 排除；技能评估产物位于 `target/skill-evals/`。
- 检查：`git ls-files .trellis .agents .pi`、`git status` 与提交文件列表；工具文件可由普通 clone/checkout 取得。

### F20 本地 Windows 包

- 契约：本机产出未签名 NSIS 与 portable zip；版本取 `VERSION`，本地构建不配置更新密钥时自动更新关闭。发布签名流程另按发布授权执行。
- 必需内容：`winc.exe`、`winc-pty.exe`、`conpty.dll`、`OpenConsole.exe`、`winc-watch`、`winc-server-Linux-x86_64`、`winc-server-Linux-aarch64`、图标、启动脚本、许可证；portable 还有 data 目录说明。
- 构建：Windows release 主程序/pty、musl watch、`cargo xtask server`、`cargo xtask icons`、PowerShell 7 的 `release/windows/conpty.ps1`、`release/windows/package.ps1 -Phase portable`、NSIS。参数与产物路径见 [Windows 打包步骤](windows-packaging.md)。
- 约束：`RUNNER_TEMP/build-config.json` 的 version 与仓库一致；NSIS 四段 `VIProductVersion`；Git Bash 调 makensis 时 `MSYS2_ARG_CONV_EXCL='*'`；网络命令带超时，长构建后台运行。
- 检查：版本资源、压缩包测试、包内必需文件和主程序哈希。`release/windows/verify.ps1` 会安装并改注册表/Path，执行需要对应授权。

## 当前契约复核项

下表描述定位基线的代码和覆盖边界。每轮先与合并前源码核对，不能把缺口改写成满足契约，也不能自动把所有缺口算作本轮回归。

| 关联 ID | 当前代码/覆盖边界 | 合并验收要求 |
| --- | --- | --- |
| F01/F05 | `crates/winc-app/src/winc.rs`::new_window 的已开目录分支仅 raise；open_targets 直接改 active，window_for 只选第一个目录，后续 open_files 跳过目录 | 分别检查初次 CLI、跨进程多个目录、已有后台目录与文件；所有入口实际移交窗口 |
| F02/F03 | `crates/winc-app/src/app/input/mouse/click.rs` 中项目条目先于 settings/form-dialog 拦截；input/keyboard.rs 的 Escape 只取消 tree drag；projects_reveal 无置 true 路径 | 模态输入不穿透，Escape 取消项目拖动，切换后当前项滚动可见 |
| F06 | keymap/action.rs 与 context menu 缺少独立 CloseProject 命令；settle_projects 选择 windows.last() | 保留命令/右键入口与相邻项目选择契约，当前差距逐项记录 |
| F07 | `crates/winc-app/src/app/lifecycle/closing.rs`::request_close_project 检查 format.save_pending；request_quit 直接确认/关闭；`crates/winc-app/src/winc.rs`::new_window 无 quitting guard | 普通保存、格式化期间退出有顺序；所有新建入口在退出期间受控 |
| F07 | `crates/winc-app/src/winc.rs`::ask_to_quit 未先 activate；无 PendingUi 调度；无窗口时 terminal 确认返回 false、格式化审批报错、保存提示直接返回；app/git/worktree.rs::confirm_remove_worktree 无窗口时直达删除 | 各后台提示先获得所属窗口，取消保留项目；worktree 删除确认保留 |
| F07 | `crates/winc-app/src/app/workspace/files.rs`::on_picked 对 SaveAs 先取 closing.continuation，无 picker identity；普通 SaveAs 未纳入 closing_pending | 旧 picker 回调不消费新关闭请求，另存为期间关闭按所属操作排队 |
| F08 | `crates/winc-app/src/winc.rs`::blank_window 只看目录/文档；restore_windows 可复用 rootless editor；`crates/winc-editor/src/backup.rs` 中 folderless 项目共用 empty 路径 | 开始页保留终端与草稿，多个未命名项目分别恢复，锁占用有明确归属 |
| F09 | `crates/winc-app/src/app/git/commit/state.rs`::CommitScope::of 在空索引选 All(tracked)，actions.rs 设置 options.all；现有测试预期 Commit All | 单独记录原有“空索引提交先提示”与当前行为差距 |
| F11 | `crates/winc-app/src/app/git/commit/state.rs`::stages_by_hunk 无 symlink 排除；`crates/winc-editor/src/git.rs`::file_mode 只返回 100644/100755 | 真实链接用例证明整文件保留链接模式、部分暂存入口受限 |
| F11/F12 | `crates/winc-app/src/app/git/commit.rs` 中 CommitName::File(PathBuf) 未含 CommitGroup；StageJob 为本 CommitState 内队列，无旧共享 worktree claim/release | 已暂存/未暂存同路径分别定位，多项目操作同仓库有占用和释放证据 |
| F13 | `crates/winc-app/src/app/git/branch.rs`::branch_ops 中 CopyName 仍落入 !busy 限制；现有菜单分组测试使用 busy=false | Git 操作期间复制名称可用，另核对剪贴板执行 |
| F15 | 旧 job 测试只覆盖编号不匹配；formatting_done 将 sent 文本传给 Document::apply_formatted | 文档再次编辑的处理沿实际应用逻辑检查；测试辅助 wait_formatting 也传递同一 job |
| F01/F07/F16 | `crates/winc-app/src/app/lifecycle/events/tests.rs`、lifecycle/window/tests.rs、terminal/tests.rs 父模块未加载，内容仍用 PendingUi/take_shell_state 等旧 API | 用 mod/cfg 和测试枚举确认覆盖，文件存在不算覆盖 |
| F01/F05 | `crates/winc-app/src/app/snapshot/projects.rs` 为离屏测试，点击只检查 Focus 请求；session 的多目录测试只检查启动解析列表 | 实际窗口交接、转发多目录与跨进程路径分别取得行为证据 |
| F07/F16 | `crates/winc-app/src/app/lifecycle/closing.rs` 的 shared.others() 先走 CloseWindow；events.rs 的 macOS 关闭回调捕获创建时项目 proxy；place_traffic_lights 无项目栏偏移 | 多项目系统关闭遵守平台契约，macOS 回调指向持窗项目，标题栏位置和隐藏后的终端焦点正确 |
| F17 | `crates/winc-app/src/app/git/commit/state.rs` 与 view/git/commit/row.rs 显示路径未统一斜杠；多数旧 LF fixture 配置和 the_refresh_button_spins_for_at_least_one_turn 的等待稳定化未保留 | Windows 路径显示、主机 CRLF、异步等待分别检查，与旧行为及测试条件对照 |

## 验收方法

1. 合并前与合并后各保存 F01–F20 的源码、入口、默认值、已加载测试、运行结果；上游接管的实现照样填写行为结果。
2. 对照当前 fork 提交集合与差异，覆盖无冲突文件；参考实现改名时，更新定位信息，不删行为契约。
3. 先跑 `cargo test -p winc-app -- --list` 并确认具体测试名；核心暂存用 `cargo test -p winc-core`，双语键集用 `cargo test -p winc-i18n`，再执行 workspace 门禁。
4. Unix-only 测试在 Unix 环境运行；musl 的 `cargo check --all-targets` 只检查编译。窗口、原生对话框和剪贴板分别记录运行环境和实际结果。
5. 自动化或环境覆盖不足的项目标成待验证；本轮改动引入的回归保持未通过状态。功能移交上游只在行为、入口、默认值、边界都一致时记「上游等价」。

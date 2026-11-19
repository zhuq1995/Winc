# Winc fork 功能保留清单

本清单以应用代码 `d54201a`、上游 `v0.1.733` 为定位基线。行为契约在实现改名后仍适用；代码位置随合并更新。下文路径均相对于应用仓库根目录。

## 目录

- [功能索引](#功能索引)
- [项目与窗口 F01–F08](#项目与窗口-f01f08)
- [Git 与菜单 F09–F14](#git-与菜单-f09f14)
- [格式化与平台 F15–F17](#格式化与平台-f15f17)
- [仓库与发布 F18–F20](#仓库与发布-f18f20)
- [顶栏与侧栏控件 F21](#顶栏与侧栏控件-f21)
- [回归覆盖边界](#回归覆盖边界)
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
| F21 | 项目切换时原生命中更新句柄随系统窗口交接，侧栏宽度变化后控件继续接收点击 | `crates/winc-app/src/app/lifecycle/window.rs` |

## 项目与窗口 F01–F08

### F01 单窗口与窗口交接

- 契约：同一普通实例的项目共用一个系统窗口；每个项目保留文档、标签、编辑状态、终端连接与后台任务。
- 实现：`crates/winc-app/src/winc.rs` 的 `Slot`、`Project`、`Shared::projects`、`Winc::activate`；`crates/winc-app/src/app/lifecycle/window.rs` 的 `has_window`、`take_window`、`put_window`、`take_window_state`、`put_window_state`；`crates/winc-app/src/app.rs` 的项目字段。
- 交接内容：窗口句柄与 `FrameState.renderer` 成对移动，显示链接和 macOS traffic lights 随窗口移动；保留 frame、controls、主题、修饰键、鼠标及焦点，清理拖动、悬停、提示、IME 预编辑与终端旧焦点。
- 打开目录、外部传入路径与重选已开项目都经 `Winc::activate` 移交窗口。`App::resumed` 只对当前项目创建窗口。
- 检查：A→B→A 后窗口句柄与 renderer 身份相同，两个项目的标签和光标独立，终端进程持续输出。原生交接的回归见 F21。

### F02 项目列表布局与输入

- 契约：项目数大于 1 时左侧列表宽 `PROJECTS_WIDTH = 190.0`，否则为 0；列表与侧栏、标题栏控件及主区分隔。
- 实现：`crates/winc-app/src/layout.rs` 的 `Layout::projects`、`PROJECT_ROW = 30.0`、`PROJECT_GAP = 2.0`、`PROJECT_PAD = 6.0`、`projects_max_scroll`；`crates/winc-app/src/theme.rs`；`crates/winc-app/src/view/workspace/projects.rs`。
- 输入：`crates/winc-app/src/app/input/mouse/regions.rs` 的 `project_list_at`、`project_at` 校验布局和 `projects_keys`，关闭按钮优先。设置、提示框和表单接收自己的点击与滚动。侧栏点击先关闭 picker，再接受下一次切换。
- `crates/winc-app/src/app/scroll/wheel.rs::scroll_projects` 限制滚动并清除旧命中；`Winc::activate` 请求显示当前条目。
- 测试：`crates/winc-app/src/app/snapshot/projects.rs` 统计单个和多个项目的列宽，覆盖滚动、窄视口、模态与 picker；`crates/winc-app/src/layout/tests.rs` 检查布局。

### F03 拖拽排序

- 入口：左键按住项目条目上下拖动；普通点击切换。
- 实现：`crates/winc-app/src/app.rs::Drag::Project`；`crates/winc-app/src/app/input/mouse/drag.rs` 的阈值 4.0；`crates/winc-app/src/app/input/mouse.rs` 在松开时把间隙编号换成最终下标；`crates/winc-app/src/winc.rs` 的 `Request::MoveProject` 与 `move_project`。
- 契约：当前项目与终端保留；原位松开不重排，Escape 取消按下或拖动，移出列表或来源行失效时放弃落点。拖拽指示与释放使用相同的当前滚动位置。
- 测试：`cargo test -p winc-app --lib snapshot::projects`，覆盖拖动、Escape、滚动后落点和过期命中；退出后恢复的项目顺序见 F08。

### F04 项目名称与标记

- 实现：`crates/winc-app/src/winc.rs` 的 `project_label(root: &Path) -> (String, String)` 与 `refresh_shared`；`crates/winc-app/src/view/workspace/projects.rs` 绘制徽标、名称、忙碌圆点与悬停关闭按钮。
- 契约：使用文件夹名，同名目录显示 `parent/name`；无目录显示开始页；长名称截断、当前项高亮。忙碌项未悬停时显示圆点，关闭按钮在悬停时显示。
- 文案：`crates/winc-i18n/locales/en-US/workspace.ftl` 与 `crates/winc-i18n/locales/zh-CN/workspace.ftl` 的 `workspace-projects-start-page`。
- 测试：`crates/winc-app/src/winc/tests.rs::project_names`；`crates/winc-app/src/app/snapshot/projects.rs::projects_with_the_same_folder_name_show_their_parent`。

### F05 打开目录与 worktree

- 契约：CLI、进程转发和系统打开路径中的多个目录各开一个项目，窗口交给第一个目录所在项目。已开的目录重用原项目，单独传入文件时优先交给包含它的最内层项目。选择其他 worktree 保留当前项目。
- 实现：`crates/winc-app/src/lib.rs::open_targets` 返回额外目录；`crates/winc-app/src/winc.rs` 的 `open_projects`、`restore_windows`、`open_targets`、`window_for`、`project_for` 和 `activate`；`crates/winc-app/src/app/git/location_picker.rs` 的 `open_folder(&place.path)`；`crates/winc-app/src/app/git/worktree.rs::open_worktree`。
- 检查：启动参数含两个目录、转发两个目录、重开已有后台目录、打开后台项目内文件、打开另一个 worktree，逐项核对窗口归属及已开标签。

### F06 项目关闭

- 入口：项目条目右侧 ✕，以及项目名称菜单和命令面板的「关闭项目」。单项目时名称菜单和命令仍可用，独立临时实例沿用关闭目录与窗口。
- 实现：`crates/winc-app/src/winc.rs` 的 `Request::CloseProject`、`close_project`、`settle_projects`；`crates/winc-app/src/app/lifecycle/closing.rs` 的 `CloseRequest::CloseProject`、`request_close_project`、`finish_close`；`crates/winc-app/src/keymap/action.rs::Action::CloseProject`。
- 契约：保存与终端确认归目标项目，终端确认使用「关闭项目」文案。关闭当前项后优先选择下一项，末项选择上一项，最后一个项目停在开始页。
- 测试：`crates/winc-app/src/app/snapshot/projects.rs` 的项目关闭测试，`crates/winc-app/src/app/snapshot/workspace/session.rs::close_project_entries_work_with_one_project` 检查名称菜单、命令面板、取消与放弃。

### F07 提示归属、保存与退出

- 契约：提示由所属项目承载，窗口同一时刻归一个项目；原生另存为、macOS sheet、终端确认、格式化审批与 worktree 删除确认都检查项目归属。
- 实现：`crates/winc-app/src/app/lifecycle/closing.rs` 的 `deferred_close`、`defer_close`、`resume_deferred_close`、`closing_pending`；`crates/winc-app/src/app/editor/format.rs::save_with_notes` 完成保存后继续关闭，保存失败清除推迟的关闭。
- 契约：保存/格式化期间关闭项目只排队一次，退出优先于关闭项目；取消或失败后保留文档与项目；退出进行中阻止全部新建项目入口。
- 系统窗口关闭：Windows/Linux 的 `request_close_window` 直接调用 `request_quit`，由 `Winc::advance_quit` 确认全部项目。取消退出保留全部项目。macOS 普通会话隐藏窗口。
- 退出提示归属：`Winc::ask_to_quit` 先 `activate(key)`，再请求该项目退出；全部确认完成后接回发起退出时的项目，保留其 `State::last` 恢复顺序。
- 生命周期定位：`crates/winc-app/src/winc.rs` 的 `Quitting::active` 保存退出发起项目，`advance_quit` 在所有确认完成后恢复它；事件循环退出后，`Winc::exiting` 完成各项目的退出保存。
- 原生提示回归：`crates/winc-app/src/app/snapshot/windows/native_input.rs::project_switch_transfers_native_hit_test_updates` 同时检查后台项目确认时接到原系统窗口、保存提示可见，以及取消后全部项目继续保留。
- 回调归属：`crates/winc-app/src/app/workspace/files.rs` 的 `Pick::SaveAsClosing` 只续接对应文档的关闭提示，普通 `SaveAs` 保留待处理的关闭请求。`crates/winc-app/src/app/git/worktree.rs::confirm_remove_worktree` 在后台项目保留应用内确认，确认前保留目录和分支。无窗口确认覆盖见 `crates/winc-app/src/app/git/worktree/tests.rs`。
- 测试：`closing_unsaved_tabs_asks_save_discard_or_cancel`、`worktrees_are_created_removed_and_pruned`（Unix Git snapshot）、`closing_the_shared_window_waits_for_background_project_confirmation`，以及 F21 原生交接。

### F08 现场、备份与独立进程

- 契约：目录、标签和目录备份按项目保存；开始页只有在无目录、无文档、无终端以及无待处理关闭时才可复用，优先用当前空项目。无目标参数的普通启动若恢复到目录，则从 `backups/empty` 恢复有内容的后台开始页，目录保持当前项目。
- 实现：`crates/winc-app/src/winc.rs` 的 `remember_windows`、`restore_windows`、`restore_folderless_project`、`is_blank`、`blank_window`；`crates/winc-app/src/app/workspace/session.rs`；`crates/winc-editor/src/backup.rs` 的 `Backups::new`、`Backups::lock`；`crates/winc-app/src/lib.rs` 与 `crates/winc-app/src/main.rs` 的启动及 one-off 路径。
- `--new-instance`、`--wait`、标准输入使用独立实例语义；one-off 不污染普通项目的最近列表、恢复与备份。
- 测试入口：`crates/winc-app/src/app/snapshot/workspace/session.rs`、`crates/winc-app/src/app/lifecycle/one_off/tests.rs`；先确认相应模块与平台条件被加载。
- 退出保存顺序：`crates/winc-app/src/app/lifecycle/events.rs::prepare_exit` 为各项目保存现场，当前项目最后保存；`Winc::remember_windows` 随后写入其他项目。状态文件的 `last` 指向退出发起项目，`otherWindows` 保存其余目录，目录现场保存标签路径、顺序和活动标签；启动沿 `restore_windows` 与 `restore_at_launch` 恢复。
- 多项目恢复回归：`crates/winc-app/src/app/snapshot/windows.rs::closing_the_shared_window_preserves_projects_and_tabs_for_restart` 走系统关闭入口，断言事件循环退出、全部项目保留到退出阶段，读回状态文件并恢复两个项目的标签和活动标签；`closing_the_shared_window_waits_for_background_project_confirmation` 覆盖后台项目取消与继续退出。

## Git 与菜单 F09–F14

### F09 真实索引

- 暂存和取消暂存与终端共享 Git 索引。默认暂存区视图从索引提交，显示列表截断时仍读取真实提交范围。空索引且当前仓库有改动时，普通提交、提交并推送和提交并同步均确认暂存全部非忽略的更改，含未跟踪文件。取消保留原状态，提交失败保留索引和草稿。
- `crates/winc-app/src/app/git/commit/actions.rs` 的 `git_commit_request`、`confirmed_commit`、`commit_with_status` 管理范围与续接。`crates/winc-editor/src/git.rs` 的 `Git::commit_status` 读取完整状态。`crates/winc-editor/src/git/commit.rs` 的 `Git::stage_all_if_empty` 在暂存前再次检查索引，`Git::commit_with` 执行提交。
- 当前部分暂存计算三份文本，调用 `stage_content` 写入索引内容；合并按行为验收，不要求固定为 `git apply --cached`。
- 检查同一文件两处改动时只暂存一处，提交后用 `git diff` 检查另一处。运行 `cargo test -p winc-app --lib directory_staging::save_and_commit`，覆盖确认、忽略文件范围、先保存后暂存、索引变化、钩子拒绝和推送／同步续接。

### F10 默认暂存区与按钮

- 默认：`git.stagingArea` 缺省即开启，显式 `false` 关闭。以下四处都是 `git_staging_area != Some(false)`：
  - `crates/winc-app/src/lib.rs`：首次启动。
  - `crates/winc-app/src/winc.rs::add_project`：新项目初始化。
  - `crates/winc-app/src/app/git/commit/staging.rs::apply_staging_view`：应用设置。
  - `crates/winc-app/src/app/settings.rs`：设置选中状态。
- `crates/winc-config/src/config.rs::Config::git_staging_area` 的注释与四处一致。
- 入口：设置 › Git › 显示暂存区；提交视图 ⋯ 菜单的 `CommitStagingArea`／`CommitChangeList`；`toggle_staging_area`。
- UI：暂存区视图按已暂存/未暂存分组，文件行尾 `CommitTarget::StageFile(index, on)` 显示 ＋／−；`crates/winc-app/src/view/git/commit.rs::file_action_targets` 负责命中。差异块/行采用上游 `DiffTarget::Include`／`IncludeLine` 复选框。
- 目录行尾 `CommitTarget::DirStage(row, on)` 与分组标题 `GroupStage(group)` 同样提供 ＋／−；目录范围来自 `commit_rows().dir(group, dir)`，包含折叠的子目录且只取当前组内文件。长目录名展开时按钮随全名移动，包含顶部固定目录；名称裁剪、提示与点击命中保持一致；历史浏览、勾选式和冲突目录隐藏暂存按钮。
- 目录回归入口：`crates/winc-app/src/app/snapshot/directory_staging.rs` 使用真实 Git 索引和离屏鼠标操作；检查目录外已暂存文件、其他分组未跟踪文件及磁盘内容保持原状，并检查长名展开与固定目录按钮的命中。
- 「更改」组的文件、目录、组标题和「放弃所有更改」按选中组或受跟踪的未暂存改动回滚。`crates/winc-app/src/app/git/commit/actions.rs` 的 `rollback_request`、`git_rollback` 校验发起仓库和范围，`crates/winc-editor/src/git/commit.rs` 的 `Git::restore_worktree` 从索引恢复磁盘。未跟踪单项和暂存删除后重建的同名文件送废纸篓。勾选式回滚使用 HEAD。
- 运行 `cargo test -p winc-app --lib directory_staging`，检查同一文件两组的右键和键盘、折叠目录、多选、批量放弃以及暂存新增、改名、删除后重建。对照磁盘内容、索引和 HEAD。
- `GitStageAll`、`GitUnstageAll`、`GitStageFile` 是整文件命令。`stage_all` 暂存列表中的受跟踪改动，保留已暂存删除及重建的同名未跟踪文件。当前文件命令和文件树、标签页右键支持只在编辑器中修改的文件。
- 检查：全新配置首次启动、第二个项目、切换设置、重启四条路径；显式 false 仍可切到勾选式。只保留核心索引代码不能证明默认视图或入口保留。

### F11 部分暂存与边界

- 实现：`crates/winc-core/src/diff/staging.rs` 的 `stage`、`unstage`、`staged_picks`；`crates/winc-app/src/app/diff.rs::toggle_diff_part`；`crates/winc-app/src/app/git/commit/state.rs::stages_by_hunk`；`crates/winc-app/src/view/diff.rs`。
- 整文件、整块、单行暂存／取消暂存共用真实索引。提交视图以 `(CommitGroup, 路径)` 区分同一路径的两组行，右键和空格键按选中组执行。历史差异隐藏暂存控件并移除命中区域。
- 契约：部分操作根据 Git 对象模式与磁盘类型拒绝符号链接，包括 `core.symlinks=false` 的普通文件检出。支持整文件暂存，保留链接模式；二进制、非 UTF-8 和冲突条目沿用各自边界。
- 实现：`crates/winc-editor/src/git/parse.rs` 读取状态中的 `120000` 模式，`crates/winc-editor/src/git/commit.rs::Git::check_partial_path` 在部分读写前校验路径。选择的文件路径被普通目录替换时拒绝整文件暂存与取消暂存；路径按 `:(literal)` 传入。
- 保留上游唯一实现，禁止恢复 `crates/winc-editor/src/git/staging.rs`、`crates/winc-app/src/app/git/commit/stage.rs` 两份并行实现。
- 测试：`partial_staging_refuses_a_link_checked_out_as_a_regular_file`、`staging_a_file_replaced_by_a_directory_does_not_stage_its_children`，以及核心部分暂存和真实仓库路径用例。

### F12 暂存/提交队列与资源

- `crates/winc-app/src/app/git/commit/staging.rs` 的 `StageJob::{Add,AddAll,Remove,Part}`、`push_staging`、`pump_staging`、`staging_finished` 维护索引队列，加项在目标文件保存成功后入队。`crates/winc-app/src/app/git/commit.rs` 的 `commit_queued` 记录提交请求与推送／同步意图。
- 当前项目的索引写入串行，提交等待写入结束。失败时清队列并按真实状态刷新。不同项目的同一仓库及终端写入可以交错。指定路径写入和回滚前校验工作树根，缺失时停止。
- `CommitState::staging()` 表示视图开关，不能作为写入互斥标志；`staging_finished` 根据 root 匹配结果归属。
- 检查：连续点暂存后提交、索引锁失败、写入中切项目/关闭项目、操作完移除 worktree；检查内容与锁文件，不能只看 busy 图标。

### F13 复制分支名

- 实现：`crates/winc-app/src/app/git/branch.rs` 的 `BranchOp::CopyName`、`GROUPS`、当前分支过滤、`label`、`run_branch_op`。
- 契约：分支操作菜单有「复制分支名」，含当前、本地与远程分支；Git 忙碌时复制入口仍可用；复制真实名称到剪贴板，无换行且不执行切分支。
- 文案：`crates/winc-i18n/locales/en-US/git.ftl` 与 `crates/winc-i18n/locales/zh-CN/git.ftl` 的 `git-branch-copy-name`。
- 测试：`crates/winc-app/src/app/git/branch/tests.rs::branch_ops_are_grouped_with_separators` 检查分组，`copy_branch_name_stays_enabled_while_busy` 检查当前、本地和远端项的可用性以及剪贴板文本。

### F14 复制工作树名

- 实现：`crates/winc-app/src/app/input/context_menu.rs::MenuCommand::LocationCopyName`；`crates/winc-app/src/app/input/context_menu/present.rs` 的位置菜单；`crates/winc-app/src/app/input/context_menu/run.rs` 调用 `winc_editor::git::repo_name(&path)` 后 `set_clipboard(name, false)`。
- 契约：位置/worktree 菜单有「复制工作树名」，复制目标工作树名；保持与复制分支名、复制路径不同的入口。
- 文案：`crates/winc-i18n/locales/en-US/git.ftl` 与 `crates/winc-i18n/locales/zh-CN/git.ftl` 的 `git-location-copy-name`。
- 测试：`crates/winc-app/src/app/git/location_picker/tests.rs::location_menu_copies_the_selected_worktree_name`，检查主工作树与附属工作树的菜单目标和剪贴板内容；双语键集由 `winc-i18n` 检查。

## 格式化与平台 F15–F17

### F15 格式化保存作业号

- 实现：`crates/winc-app/src/app/editor/format.rs` 的 `FormatState::next_id`、`Running.job`、`Outcome = (DocumentId, u64, Result<Formatted, FormatError>)`、`launch`、`poll_formatting`、`formatting_done`、`ToolAnswer.save_id`、`answer_tool_choice`、`saving`、`save_with_notes`。
- 契约：结果按文档 ID 与 job 匹配；同文档新作业不会被旧结果覆盖；保存审批按 save_id 匹配；取消清除对应记录，失败保留文档并清除推迟关闭。
- 测试：`crates/winc-app/src/app/editor/format/tests.rs::a_stale_format_result_is_discarded`；`crates/winc-app/src/app/snapshot/editor/format.rs`。当前模块为 Unix 条件，Windows 的测试通过不包含它们。
- 检查：旧结果、新结果交错；旧审批晚于新保存；格式化失败、保存失败、保存中关项目；编号匹配与文档内容版本检查分别核对。

### F16 平台窗口与终端焦点

- 契约：macOS 的窗口句柄从 `App.window.handle` 获取；traffic lights、显示链接、IME 和主题随实际窗口交接；窗口切走时解除终端前台焦点，终端连接保留。
- 平台关闭契约：macOS 普通会话隐藏窗口并保留项目；临时会话沿用窗口关闭流程；Windows/Linux 关闭窗口走全部项目的退出确认。
- 实现：`crates/winc-app/src/app/lifecycle/window.rs`、`crates/winc-app/src/app/lifecycle/events.rs`、`crates/winc-app/src/app/lifecycle/closing.rs`、`crates/winc-app/src/app/terminal.rs`。
- 检查：各平台确认窗口交接、系统关闭、重新打开和终端焦点。以父模块 `mod` 和 `cfg` 确认测试是否加载。

### F17 可重复的本地测试

- 契约：临时仓库在首个 `git add` 前设置 `core.autocrlf=false`。Windows Git 子进程无额外控制台。动画测试冻结时钟，Markdown 同步首帧测试固定上一次解析耗时。提交列表目录标签采用 `/`。
- 实现：`crates/winc-app/src/app/snapshot/git.rs` 的公共 fixture、`crates/winc-editor/src/test_util.rs::quiet` 与对应 rebase/sync/worktree fixture；`crates/winc-app/src/app/git/commit.rs` 的提交行路径格式化。
- 测试：`commit_row_labels_use_forward_slashes`、Unix 的 `the_refresh_button_spins_for_at_least_one_turn` 和 `crates/winc-app/src/app/snapshot/preview/live.rs::typing_shows_the_final_layout_in_the_same_frame`。测试 PATH 包含 Git `cmd`，临时库配置与调用者的 Git 环境配置独立保留。

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

## 顶栏与侧栏控件 F21

### F21 标签栏与侧栏分段的命中区域

- 契约：Windows 原生命中测试的更新句柄 `WindowInput` 属于系统窗口，随 `Shell` 交给当前项目。切换项目后，当前项目重绘并把顶栏和侧栏控件的位置同步到已安装的 `WM_NCHITTEST` 回调。
- 实现：`crates/winc-app/src/winc.rs` 的 `Shell::window_input` 保存句柄；`crates/winc-app/src/app/lifecycle/window.rs` 的 `take_window` 用 `self.touch.window_input.take()` 移出，`put_window` 接回 `self.touch.window_input`；`crates/winc-app/src/app/input/mouse/target.rs` 的 `sync_window_input` 更新回调共享的命中区域。
- 边界：窗口子类回调与 `WindowInput` 持有同一份 `Arc<Mutex<WindowHitTest>>`。切走的项目释放更新句柄的所有权，当前项目负责后续更新。窗口交接保留该句柄，禁止只交接窗口与渲染器。
- 表现：系统按当前项目的按钮位置分发客户区输入；侧栏拖宽或收窄后，标签 `✕`、`+` 和 Files/Search/Commit 分段继续响应点击。
- 图标：`crates/winc-app/src/view/workspace/panel.rs` 的 `VIEW_ICON` 是倍率，常态为 `0.93`；传给 `icons::files/search/git` 的参数按倍率解释。
- 测试：`crates/winc-app/src/app/snapshot/windows/native_input.rs::project_switch_transfers_native_hit_test_updates` 创建真实 Windows 窗口，执行 A → B → A 的窗口交接，改变侧栏宽度，通过 `SendMessageW(WM_NCHITTEST)` 检查已安装回调对标签关闭、新建和侧栏分段返回 `HTCLIENT`。
- 检查：在两个项目之间往返切换；分别拖宽、收窄侧栏；点击标签关闭、新建以及 Files/Search/Commit；核对图标尺寸。回归测试需要查询已安装的系统回调。

## 回归覆盖边界

| 关联 ID | 边界 | 检查 |
| --- | --- | --- |
| F01/F05/F07/F16/F21 | 离屏测试不建系统窗口；macOS sheet、窗口关闭回调和终端焦点依赖操作系统。 | Windows 用 F21 查询实际窗口回调；macOS 运行切换、关窗后重开和提示用例；按测试枚举确认平台条件。 |
| F07 | 后台 worktree 移除提示可保留到项目重新接窗；另存为、保存审批和原生 sheet 分别路由。 | 逐项从发起项目操作，切换后检查取消与完成的归属。 |
| F08 | 无目录项目共用 `backups/empty` 与一个独占锁。 | `folderless_backups_restore_beside_the_last_folder` 检查目录启动时恢复后台开始页。未持有锁的项目沿退出保存确认处理。 |
| F09/F11/F12 | 空索引检查与 `git add --all` 分两步执行；路径验证与 Git 执行之间允许外部文件系统和索引变化。 | 保留真实索引、队列、文件替换和错误反馈检查，终端并发单列。 |
| F15/F17 | 格式化脚本与部分 Git snapshot 使用 Unix 测试，Windows 不枚举这些用例。 | Unix 运行格式化作业号、工作树确认与刷新动画测试。 |

## 验收方法

1. 合并前与合并后各保存 F01–F21 的源码、入口、默认值、已加载测试、运行结果；上游接管的实现照样填写行为结果。
2. 对照当前 fork 提交集合与差异，覆盖无冲突文件；参考实现改名时，更新定位信息，不删行为契约。
3. 先跑 `cargo test -p winc-app -- --list` 并确认具体测试名；核心暂存用 `cargo test -p winc-core`，双语键集用 `cargo test -p winc-i18n`，再执行 workspace 门禁。
4. Unix-only 测试在 Unix 环境运行；musl 的 `cargo check --all-targets` 只检查编译。窗口、原生对话框和剪贴板分别记录运行环境和实际结果。
5. 自动化或环境覆盖不足的项目标成待验证；本轮改动引入的回归保持未通过状态。功能移交上游只在行为、入口、默认值、边界都一致时记「上游等价」。

### Windows 窗口与退出专项回归（F07/F08/F21）

窗口交接、原生输入、系统关闭或现场保存路径受合并影响时，执行以下检查：

```bash
cargo test -p winc-app --lib project_switch_transfers_native_hit_test_updates
cargo test -p winc-app --lib closing_the_shared_window_
```

- 原生输入：测试经 `crates/winc-app/src/app/snapshot/windows.rs` 的 `#[cfg(windows)] mod native_input` 加载。A → B → A 交接后，改变侧栏宽度，向已安装的 `WM_NCHITTEST` 回调查询 `✕`、`+` 与 Files/Search/Commit 中心，结果为 `HTCLIENT`；图标参数保持倍率语义。
- 后台提示：退出时目标项目接到原窗口，保存提示显示；取消保留所有项目。全部确认后，当前项目恢复为退出发起项目。
- 真实关闭与重启：用独立 portable `data` 目录隔离用户状态与单实例通道，开启两个项目，每个项目两个文件标签和终端，选择非首个标签。关闭实际编辑窗口后检查本实例的 `winc.exe`、`winc-pty.exe` 全部退出；无参数重启，核对两个项目、标签顺序、活动标签和当前项目。连续执行两轮。
- 消息目标：自动化向可见的编辑窗口发送 `WM_CLOSE`，以进程 ID、窗口类 `Window Class` 和非空标题确认目标；`Winit Thread Event Target` 属于消息辅助窗口。主窗口句柄在启动完成后重新获取。
- 包产物：按 F20 核对 NSIS、portable 内的主程序与本轮 release 构建哈希一致。

# 单窗口多项目：窗口交接约定

一个 winc 进程只有一个系统窗口，窗口里按侧栏列表切换多个项目。改窗口、事件路由或项目状态前先读这一页。

## 资源归属

- `crate::winc::Shell` 持有系统窗口、渲染器、红绿灯、显示链接和“被遮住”状态：同一时间只交给一个项目。
- Windows 的 `WindowInput` 更新句柄属于系统窗口，随 `Shell` 经 `App::take_window` / `App::put_window` 交接。当前项目每帧把控件位置同步给已安装的 `WM_NCHITTEST` 回调；后台项目不保留更新句柄。回归测试通过真实窗口的系统回调查询控件区域。
- `App` 每个项目一个实例，持有编辑器、文档、终端及其辅助进程连接、文件树、搜索、Git、备份和项目自己的界面状态。
- 切换项目（`Winc::activate`）用 `App::take_window` 取出旧项目的 `Shell`，用 `App::put_window` 接到新项目。保留两个项目的运行状态，窗口交接完成后请求显示当前条目。
- `App::request_close_project` 的保存与备份流程完成后置 `project_closed`。`Winc::settle_projects` 优先激活下一项，没有下一项时选上一项，再移除已关闭项目。

## 事件路由

- 窗口事件（`Winc::window_event`）只交给持有窗口的项目：系统窗口只有一个，`key_of` 找不到时没有收件人。
- 后台事件按 `Recipient::Window(key)` 回到发起它的项目；项目切换不影响这些事件的投递。
- 非当前项目不绘制（`App::redraw` 没有渲染器时直接返回），但终端消息、文件监视和备份继续处理；终端尺寸在它重新接上窗口、`relayout` 之后同步。
- 项目关闭后仍在路上的结果由 `Winc::recipient` 丢弃，不能落到其他项目上。

## 布局与界面

- `Layout::compute` 的 `projects_width` 决定最左侧项目列表是否占位置：只有多个项目时非零（`App::projects_width`）。
- 项目列表的绘制在 `view/workspace/projects.rs`，每帧把条目和关闭按钮的矩形记进 `App::projects_hits` / `App::projects_close`；点击与悬停在 `app/input/mouse` 里按这两张表判断。
- 条目按下开始 `Drag::Project`。有效区域内释放时，普通点击或原位请求 `Request::Focus`，更换间隙请求 `Request::MoveProject(key, 重排后的下标)`。Escape、移出列表和来源行失效取消拖动。关闭按钮在按下时请求 `Request::CloseProject`。
- 落点的间隙序号按 `Layout::projects`、`projects_scroll` 与 `PROJECT_ROW + PROJECT_GAP` 换算，指示线与释放共用同一落点函数。设置、提示框、表单与 picker 优先接收输入。
- 该列存在时，`Layout` 的侧栏、分隔条和编辑区整体右移 `projects_width + ISLAND_GAP`；顶栏与侧栏开关保持原来的绝对位置。

## 打开与关闭

- `App::open_folder` 在有应用层（`App::key` 非空）时只请求 `Request::OpenFolder`：已开着就切到那个项目，否则新开一个；离屏测试和测量没有应用层，直接在项目里打开。
- 新建项目复用 `Request::NewWindow`，语义是“新开一个项目”，不创建系统窗口。
- macOS 普通项目会话的 `request_close_window` 隐藏共享窗口，保留项目和标签。临时会话走单独的窗口关闭流程。Windows/Linux 用 `request_quit` 与 `Winc::advance_quit` 逐个确认各项目。
- `Winc::ask_to_quit` 先把窗口交给目标项目。取消保留全部项目，全部确认后窗口交回退出发起项目，退出现场以它作为 `State::last`。
- 关闭项目入口为侧栏 ✕、项目名称菜单与命令面板。最后一个项目关闭后留在开始页。
- `Winc::new_window` 和 `Winc::open_targets` 在退出确认期间拦截新项目和外部打开请求。
- `Pick::SaveAsClosing` 只续接所属文档的关闭请求。工作树移除确认留在发起项目，后台期间由该项目在重新接窗后显示。
- `App::take_window_state` 按离开窗口处理自动保存。
- CLI 与转发多个目录各开一个项目，首个目录所在项目持有窗口。重选已有目录走 `activate`。开始页只在文档、终端和待处理关闭为空时复用。
- 项目列表用 `App::project_label`（目录名），重名时用 `project_label_with_parent`；滚动与裁剪在 `view/workspace/projects.rs`，命中区域与之同源。

## 状态文件

- 继续用 `last` / `otherWindows` / 目录现场：当前项目的目录写 `last`，其余项目写 `otherWindows`，启动时各建一个项目。
- 普通无目标启动恢复到目录时，`Winc::restore_folderless_project` 将 `backups/empty` 中的备份恢复为后台开始页。已恢复的目录保持当前项目。无目录项目共用该备份目录的独占锁。
- 数量上限不变（目录现场 10 条、`otherWindows` 读取 10 条）；终端进程在应用退出后不恢复。
- 列表顺序取自 `Winc.windows`；拖拽排序只改内存顺序（`Winc::move_project`），`Winc::remember_windows` 退出时按新顺序写 `otherWindows`，当前项目不变。
- Windows/Linux 关闭窗口的回归从 `request_close_window` 发起，经过 `Winc::settle` 确认退出；退出阶段逐项目 `prepare_exit` 后再 `remember_windows`。测试读回磁盘状态并恢复目录、标签和活动标签，另覆盖后台项目取消退出。

# 单窗口多项目：窗口交接约定

一个 winc 进程只有一个系统窗口，窗口里按侧栏列表切换多个项目。改窗口、事件路由或项目状态前先读这一页。

## 资源归属

- `crate::winc::Shell` 持有系统窗口、渲染器、红绿灯、显示链接和“被遮住”状态：同一时间只交给一个项目。
- `App` 每个项目一个实例，持有编辑器、文档、终端及其辅助进程连接、文件树、搜索、Git、备份和项目自己的界面状态。
- 切换项目（`Winc::activate`）只做一件事：把 `Shell` 从旧项目取出（`App::take_shell`）交给新项目（`App::put_shell`）。不重建窗口、不重开目录、不序列化项目状态。
- 项目关闭（`App::request_close_project`）先走 `CloseRequest::CloseProject` 的确认与备份流程，再置 `project_closed`；`Winc::settle_projects` 取出窗口、移除项目、把窗口交给相邻项目。

## 事件路由

- 窗口事件（`Winc::window_event`）只交给持有窗口的项目：系统窗口只有一个，`key_of` 找不到时没有收件人。
- 后台事件按 `Recipient::Window(key)` 回到发起它的项目；项目切换不影响这些事件的投递。
- 非当前项目不绘制（`App::redraw` 没有渲染器时直接返回），但终端消息、文件监视和备份继续处理；终端尺寸在它重新接上窗口、`relayout` 之后同步。
- 项目关闭后仍在路上的结果由 `Winc::recipient` 丢弃，不能落到其他项目上。

## 布局与界面

- `Layout::compute` 的 `projects_width` 决定最左侧项目列表是否占位置：只有多个项目时非零（`App::projects_width`）。
- 项目列表的绘制在 `view/workspace/projects.rs`，每帧把条目和关闭按钮的矩形记进 `App::projects_hits` / `App::projects_close`；点击与悬停在 `app/input/mouse` 里按这两张表判断，命中后请求 `Request::Focus` 或 `Request::CloseProject`。
- 该列存在时，`Layout` 的侧栏、分隔条和编辑区整体右移 `projects_width + ISLAND_GAP`；顶栏与侧栏开关保持原来的绝对位置。

## 打开与关闭

- `App::open_folder` 在有应用层（`App::key` 非空）时只请求 `Request::OpenFolder`：已开着就切到那个项目，否则新开一个；离屏测试和测量没有应用层，直接在项目里打开。
- 新建项目复用 `Request::NewWindow`，语义是“新开一个项目”，不创建系统窗口。
- macOS 关闭窗口只隐藏（`close_window_now`），项目和标签保留；其他平台关闭窗口就是退出，走 `advance_quit` 逐个确认各项目。
- 最后一个项目关闭时留在开始页上，不置 `project_closed`，窗口继续显示。
- 退出确认期间 `Winc::new_window` 直接返回：新项目不进入退出队列，未保存内容与终端会被丢掉。
- 后台动作（终端确认、保存提示、格式化授权、工作树移除确认）结束时若需要窗口，先请求 `Request::Focus`，拿到窗口后再显示提示；取消则保持原状。
- 窗口交接前 `App::take_shell_state` 按“离开窗口”处理自动保存。
- 命令行多个目录：第一个留在当前项目，其余经 `Request::OpenFolder` 各开一个项目。
- 项目列表用 `App::project_label`（目录名），重名时用 `project_label_with_parent`；滚动与裁剪在 `view/workspace/projects.rs`，命中区域与之同源。

## 状态文件

- 继续用 `last` / `otherWindows` / 目录现场：当前项目的目录写 `last`，其余项目写 `otherWindows`，启动时各建一个项目。
- 数量上限不变（目录现场 10 条、`otherWindows` 读取 10 条）；终端进程在应用退出后不恢复。

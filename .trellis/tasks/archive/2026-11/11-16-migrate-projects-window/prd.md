# 单窗口多项目迁入新窗口架构

## Goal

在上游 `WindowKey`／`Recipient`／`Envelope`／`WindowProxy`／`Request` 架构上恢复 fork 的单窗口多项目：所有项目共用一个系统窗口，最左侧项目列表切换当前项目，每个项目保留自己的标签、文档、终端连接和后台任务。

## 前置状态

上游 0.1.733 的模型：`Winc.windows: Vec<Slot>`，每个 `Slot` 一个 `App`，每个 `App` 自己持有窗口（`App.window: WindowState`，其中 `handle: Option<Arc<Window>>`）与渲染器（`App.frame: FrameState`，其中 `renderer: Option<Renderer>`）。跨窗口操作用 `Request::{NewWindow(Option<PathBuf>), Focus(WindowKey), Moved(PathBuf, PathBuf)}`。

fork 原来的模型：`Winc.windows: Vec<Slot>` 复用为多项目容器，同一时间只有一个 `App` 持有 `Shell`（`window` + `renderer` + `display_link` + `traffic_lights` + `ShellState`），`activate` 时整体移交。`Shared.projects: RefCell<Vec<Project>>` 是给项目列表渲染的快照，`Project { key: WindowKey, name: Option<String>, busy: bool }`。

`Shell`／`ShellState` 在上游合并中被删除，`take_shell`／`put_shell`／`has_shell`／`wake_shell` 都不再存在。

## Requirements

- **R1 单窗口**：进程内只有一个系统窗口。打开多个目录参数（含 worktree）时，每个目录成为一个项目，不新建系统窗口。
- **R2 项目列表**：最左侧列出所有项目，点击切换，支持关闭按钮与拖拽排序。项目数大于 1 时宽度为 `PROJECTS_WIDTH`，否则为 0。
- **R3 切换保留现场**：切换项目保留窗口句柄、渲染器、字形缓存、显示链接、主题、修饰键和焦点，只换项目数据。旧项目的编辑器、终端和后台任务继续存在。
- **R4 模态界面不接收窗口**：有模态界面（提示、另存为、终端确认、macOS sheet）的项目不接收窗口，切换被拒绝。
- **R5 关闭并入推迟队列**：保存或格式化进行中的关闭请求并入 `deferred_close`，当前操作结束后执行。
- **R6 退出中不新建项目**：退出流程进行中，新的目录参数不再创建项目。
- **R7 最后一个项目停在开始页**：关闭最后一个项目时不停留在已关闭状态，回到开始页。
- **R8 跨进程打开**：另一个 `winc` 进程交来的目录在该窗口里新增项目并切换，不新建进程窗口。

## Constraints

- 只在不改变上游功能的前提下增补；上游已有的行为（文件树剪切粘贴、共享配置版本、退出询问、`Moved` 事件）保持不变。
- 不重建 `Shell` 结构体；移交的是上游已有字段（见 design.md 第 2 节）。
- 上游 `Request::NewWindow` 的语义改为「新建项目」，`Focus` 的语义改为「切到这个项目」。
- macOS 上关闭窗口只隐藏、不退出应用的行为保留。

## Acceptance Criteria

- [ ] `crates/winc-app/src/app/snapshot/windows.rs` 的项目列表用例通过。
- [ ] `crates/winc-app/src/app/lifecycle/closing/tests.rs` 的项目关闭用例通过。
- [ ] `crates/winc-app/src/app/lifecycle/window/tests.rs` 与 `crates/winc-app/src/app/snapshot/terminal/editor.rs` 的相关用例通过。
- [ ] `cargo test -p winc-app` 全绿。
- [ ] `cargo clippy --workspace --all-targets` 无警告。
- [ ] 手工验证：`winc dir-a dir-b dir-c` 只开一个窗口，项目列表有三个项目；切换三个项目时窗口不闪烁、不重建渲染器；关闭中间项目后窗口交给相邻项目；关闭到最后一个项目落到开始页。
- [ ] 手工验证：在项目 B 打开模态提示时点击项目 A，切换被拒绝，B 仍是当前项目。
- [ ] 拖拽排序后下一轮事件循环中列表顺序稳定，当前项目不变。

## Out of Scope

- 上游新功能的改写。
- 多窗口模式（每个项目一个窗口）——本任务明确保持单窗口。

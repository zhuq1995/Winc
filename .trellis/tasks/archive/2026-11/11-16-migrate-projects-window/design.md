# 设计：单窗口多项目迁入新窗口架构

## 1. 模型映射

上游合并后，窗口与渲染资源从集中的 `Shell` 变成两个 `App` 字段：

```
App.window: lifecycle::window::WindowState     // handle: Option<Arc<Window>>,
                                               // controls, focused, hidden, frame,
                                               // frame_due, occluded
App.frame:  lifecycle::FrameState              // renderer: Option<Renderer>, next_frame,
                                               // display_link (macOS/Windows), frame_interval,
                                               // bench, frame_log, presented, dirty
```

fork 原来的移交单位 `Shell` 由这两组字段拼成，因此不需要重建结构体，直接按字段移交。

| fork `Shell` 字段 | 新位置 | 移交 |
| --- | --- | --- |
| `window: Arc<Window>` | `App.window.handle` | 是 |
| `renderer: Renderer` | `App.frame.renderer` | 是 |
| `display_link`（macOS / Windows） | `App.frame.display_link` | 是 |
| `occluded` | `App.window.occluded` | 是 |
| `traffic_lights`（macOS） | 实现时定位上游对应字段 | 是 |
| `state.frame` | `App.window.frame` | 是 |
| `state.controls` | `App.window.controls` | 是 |
| `state.dark` | 上游主题字段 | 是 |
| `state.modifiers` | `App.pointer` | 否，每项目保留自己的输入状态 |
| `state.option_side` | `App.pointer` | 否 |
| `state.mouse`、`cursor_inside`、`cursor_icon` | `App.pointer` | 否 |

`Renderer` 由 `Renderer::new(window.clone(), ...)` 创建，与窗口绑定，所以 `App.window.handle` 与 `App.frame.renderer` 必须同时移交，不能只交其中一个。

## 2. 移交函数

新增三个 `App` 方法，替代被删除的 `take_shell`／`put_shell`／`has_shell`／`wake_shell`：

```rust
impl App {
    /// 取出窗口与渲染资源，交还给容器；没有窗口时为 None。
    fn take_window(&mut self) -> Option<WindowHandoff>;

    /// 接收另一个项目交来的窗口与渲染资源。
    fn put_window(&mut self, handoff: WindowHandoff);

    /// 是否持有窗口。
    fn has_window(&self) -> bool;
}
```

`WindowHandoff` 是新增的内部结构体，只承载 design 第 1 节「移交 = 是」的那些字段。它与 `Shell` 的区别：不含 `ShellState` 的输入与外观状态，那些留在各自项目里。

## 3. 容器层改动

### 3.1 `Request`

```rust
pub enum Request {
    /// 新建一个项目；带目录时在里面打开它，目录已经开在某个项目里时切到那个项目。
    NewProject(Option<PathBuf>),
    /// 把这个项目切到前台（它开着要打开的目录）。
    Focus(WindowKey),
    /// 文件或目录从 `.0` 移到了 `.1`。
    Moved(PathBuf, PathBuf),
}
```

`NewWindow` 改名 `NewProject`；`Focus` 保留，语义由「窗口切到前台」变为「项目切换」。`Moved` 不变。

### 3.2 `Winc`

`windows: Vec<Slot>` 保持，语义是项目列表；同一时间只有一个 `Slot.app` 持有窗口。

| 函数 | 来源 | 改动 |
| --- | --- | --- |
| `activate(key)` | fork 原实现 | `take_shell`／`put_shell` 换成 `take_window`／`put_window`；`wake_shell` 的分支改为「首个项目由启动流程建窗口」 |
| `close_project(key)` | fork 原实现 | 不变 |
| `move_project(key, index)` | fork 原实现 | 不变 |
| `settle_projects()` | fork 原实现 | 窗口交接部分换成 `take_window`／`put_window` |
| `refresh_shared` 中的 `Shared.projects` | fork 原实现 | 不变；`Project { key, name, busy }` 保留 |
| `Quitting` | 上游版本 | 保留；`Asking.deferred` 与 fork 的推迟语义一致 |

新增：`Request::NewProject(dir)` 的处理。上游在 `NewWindow` 里调用 `Winc::open_window` 建系统窗口；改为新建一个无窗口的 `App`（复用 `Template`），插入 `windows`，再 `activate`。

### 3.3 单窗口约束

`Winc.windows.len()` 可以大于 1，但持有 `window.handle` 的 `Slot` 恒为 1 个。加一条断言函数在调试构建里校验：

```rust
#[cfg(debug_assertions)]
fn assert_single_window(&self) {
    let holders = self.windows.iter().filter(|s| s.app.has_window()).count();
    debug_assert!(holders <= 1, "单窗口模型下最多一个项目持有窗口，实际 {holders}");
}
```

## 4. `App` 层改动

### 4.1 保留的 fork 字段与方法

| 项 | 说明 |
| --- | --- |
| `deferred_close: Option<CloseRequest>` | 推迟的关闭请求，含 `Quit` |
| `pending_ui: VecDeque<PendingUi>` | `PendingUi::{Save, Close, Format}`，移交窗口后继续的界面操作 |
| `project_closed: bool` | 标记项目已请求关闭，供 `settle_projects` 收集 |
| `projects_hits`、`projects_close`、`projects_keys`、`projects_scroll`、`projects_reveal` | 项目列表的每帧命中与滚动状态，留在 `App` 上，不并入 `App.hits` |
| `request_close_project`、`close_project_now`、`resume_deferred_close`、`resume_pending_ui` | 关闭流程 |

### 4.2 `can_handoff_for_prompt`

判定字段保持 fork 的清单，只把 `has_shell()` 换成 `has_window()`：

```rust
pub(crate) fn can_handoff_for_prompt(&self) -> bool {
    if !self.has_window()
        || self.alert.is_some()
        || self.form_dialog.is_some()
        || self.closing.prompt.is_some()
        || self.save_as_picker.is_some()
        || self.closing.confirming.is_some()
    {
        return false;
    }
    #[cfg(target_os = "macos")]
    if self.window.handle.as_deref().is_some_and(crate::macos::has_sheet) {
        return false;
    }
    true
}
```

上游新增的模态（`alert`、`notice`、`one_off` 的交互）在实现时逐个确认是否需要加入该清单。

### 4.3 启动流程

上游 `lifecycle/startup.rs` 一次完成「建 App + 建窗口」。拆成两步：

1. `App::new_project(template, dir)`：建 App，不建窗口，`window.handle = None`。
2. `App::create_window(...)`：建窗口与渲染器，填 `window.handle` 与 `frame.renderer`。

首个项目在 `resumed`／启动结束处调用两者；后续项目由 `Request::NewProject` 只调第 1 步再 `activate`。

## 5. 视图与输入

| 项 | 来源 |
| --- | --- |
| `view/workspace/projects.rs` | fork 原文件整体移植，绘制项目列表与关闭按钮 |
| `Layout::projects` 字段与 `PROJECTS_WIDTH` | fork 原实现；宽度规则：项目数 > 1 时为 `PROJECTS_WIDTH`，否则 0 |
| `place_traffic_lights` 的偏移 | fork 原实现：macOS 红绿灯随项目列表宽度右移 |
| `input/mouse/{click,drag,regions}.rs` 的 projects 命中 | fork 原实现，含拖拽排序的插入位置计算 |
| `keymap` 中的项目切换命令 | fork 原实现，与上游新增命令核对名称 |

## 6. 顺序依赖

1. `WindowHandoff` + `take_window`／`put_window`／`has_window`。
2. `Winc::activate`、`settle_projects`、`NewProject` 处理。
3. `Request` 改名与调用点。
4. 启动流程拆分。
5. 视图与输入移植。
6. `can_handoff_for_prompt` 与推迟关闭接线。
7. 测试。

## 7. 风险

| 风险 | 处理 |
| --- | --- |
| 上游在 `resumed` 里同时建窗口与填 `Renderer`，拆分后首个项目可能少建一次渲染器 | 拆分后首个项目走原路径，只对后续项目走 `new_project` + `activate` |
| 上游新增的模态界面不在 `can_handoff_for_prompt` 清单里，切换模态项目会丢界面 | 实现时穷举 `App` 中 `Option` 类型的对话框字段逐一核对 |
| 拖拽排序与上游新增的列表拖拽（如有）冲突 | 先 `rg` 确认上游无项目列表概念，命中区域独立，不冲突 |
| 测试基座依赖 `Shell` | `app/snapshot/windows.rs` 的辅助函数按新字段重写 |

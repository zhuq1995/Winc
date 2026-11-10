# 单窗口项目切换：代码调研

## 现有结构

- `crates/winc-app/src/winc.rs:1`：`Winc` 管理多个窗口，每个 `Slot` 持有独立 `App`；`WindowKey` 为后台事件提供稳定接收目标。
- `crates/winc-app/src/app.rs:677`：`App` 同时持有编辑器、终端、项目相关界面状态，以及窗口和渲染器（`:1103`）。
- `crates/winc-app/src/app/lifecycle/events.rs:16`：`App::resumed` 创建原生窗口和渲染器；`:515` 的 `tick` 包含终端、搜索等后台轮询。
- `crates/winc-app/src/winc.rs:774`：窗口事件按窗口分发，后台事件按接收目标分发；现有编号机制可用于项目事件路由。
- `crates/winc-app/src/app/lifecycle/window.rs:477`：绘制调用当前 `App` 的布局和视图。
- `crates/winc-app/src/layout.rs:103`、`crates/winc-app/src/view/workspace/sidebar.rs:35`：现有文件树、搜索与提交共用侧栏；左侧项目列表需要独立区域、绘制和鼠标命中。

## 生命周期与持久化

- `crates/winc-app/src/app/workspace/session.rs:24`：打开目录时当前可以选择新窗口或替换当前目录；`:94` 的替换路径关闭原标签和终端。
- `crates/winc-app/src/app/terminal.rs:481`：关闭终端标签会向辅助进程发送关闭请求并释放连接；切换项目时必须保留连接。
- `crates/winc-app/src/app/lifecycle/closing.rs:76`、`crates/winc-app/src/winc.rs:398`：退出和关闭有终端确认、未保存内容备份、逐窗口处理与取消流程。
- `crates/winc-config/src/state.rs:13`：最近目录及目录现场各有 10 项上限；`:388` 读取其他窗口列表时也限制为 10 项。活动目录另由 `last` 存储。
- `crates/winc-config/src/state.rs:108`：终端现场保存标签及目录等信息；应用退出后不恢复其中运行的程序。
- `crates/winc-app/src/main.rs:124`、`crates/winc-app/src/cli.rs:3`：普通命令行启动会尝试交给已运行实例；`--new-instance`、`--wait`、标准输入有独立进程语义。
- `crates/winc-app/src/instance.rs:589`：两个普通进程接近同时启动且无法获取应用锁时，未获取锁的进程仍自行打开窗口；转交失败的兜底也会自行打开。
- `crates/winc-app/src/app/snapshot/windows.rs`、`crates/winc-app/src/app/snapshot/terminal/editor.rs`：现有测试覆盖多窗口、打开目录、退出和终端切换；新行为需在这些测试基础上覆盖真实项目状态隔离。

## 涉及的平台

- macOS：标题栏、Dock、窗口关闭与系统弹窗。
- Windows：窗口控件、原生弹窗与 ConPTY。
- Linux：Wayland/X11 窗口激活、文件选择器与绘制。

## 项目约定

- `.github/CONTRIBUTING.md` 要求新增行为有测试，视觉改动使用截图测试，新界面文案同时提供 `en-US` 和 `zh-CN`。
- 项目指南 `.trellis/spec/backend/` 目前仍是初始化模板；跨层约束参考 `.trellis/spec/guides/cross-layer-thinking-guide.md`，具体代码风格参考周边代码和贡献指南。

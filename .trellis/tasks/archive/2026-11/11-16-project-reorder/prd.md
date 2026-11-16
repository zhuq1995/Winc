# 项目列表拖拽排序

## 目标

左侧项目列表（单窗口多项目侧栏）支持用鼠标拖动条目调整顺序；拖动过程中显示落点指示线，松开后按落点重排项目顺序。

## 背景

- 项目列表的绘制在 `crates/winc-app/src/view/workspace/projects.rs:12-128`，每帧把条目矩形写进 `App::projects_hits`（`:125`）、关闭按钮写进 `App::projects_close`（`:119`）。
- 列表数据由应用层从窗口顺序派生：`Winc::settle_projects` 遍历 `self.windows` 生成 `shared.projects`（`crates/winc-app/src/winc.rs:794-826`）。列表顺序即 `Winc.windows` 的顺序。
- 目前的点击行为是「按下即切换」：`crates/winc-app/src/app/input/mouse/click.rs:53-64` 在按下时直接请求 `Request::Focus`，没有拖动阶段。
- 已有的拖动实现可参照标签拖动：`Drag` 枚举在 `crates/winc-app/src/app.rs:597-720`，标签拖动的阈值与状态在 `crates/winc-app/src/app/workspace/tab_drag.rs:15-36`，鼠标松开处理在 `crates/winc-app/src/app/input/mouse.rs:93-131`。
- 退出时项目顺序由 `Winc::remember_windows`（`crates/winc-app/src/winc.rs:1107-1130`）按窗口顺序写入 `state.others`，启动时当前项目排第一、其余按记录顺序打开。因此重排 `windows` 的顺序会被现有持久化带过去，不需要改状态文件格式。

## 需求

- R1：按住项目条目并纵向移动超过拖动阈值后进入拖动状态，列表显示落点指示线（两个条目之间的间隙，或首尾位置）。
- R2：松开时按落点重排项目顺序；重排后侧栏列表、`projects_hits` 与 `projects_close` 顺序、项目切换顺序一致，当前项目保持不变（不因拖动切走）。
- R3：没有超过阈值就松开时保持原有点击语义：切换到该项目。
- R4：拖动有边界：落点限制在 `0..=项目数`，指示线裁在列表区域内；拖动期间按住 Esc 或松开鼠标都结束拖动状态，指示线消失。
- R5：不会因拖动打乱其他状态：活动项目、窗口交接、`deferred_close`、终端确认等保持不变；拖动期间关闭项目的旧路径照常工作。

## 验收标准

- [ ] R1：三个项目时把第三个条目拖到第一个之前，拖动中在第一个条目前显示一条强调色指示线；拖回原位或只轻微移动时不显示指示线。
- [ ] R2：松开后 `Winc.keys()` 的顺序变为「第三、第一、第二」，`shared.projects()` 与 `projects_hits` 顺序一致，当前项目仍是拖动前的项目。
- [ ] R3：只按下并松开（无移动）仍请求 `Request::Focus(key)`，与原来一致。
- [ ] R4：把条目拖到列表上方或下方之外，落点为第一个或最后一个；拖到列表内但落回原位时不发重排请求。按下 Esc 后指示线消失且松开不再触发重排。
- [ ] R5：重排后编辑区、终端内容不变，终端仍在运行的项目继续运行；`cargo test -p winc-app` 通过。

## 范围之外

- 排序持久化为独立的顺序字段（继续复用现有 `last` / `otherWindows` 记录方式）。
- 键盘调整顺序、项目分组、多选拖动。
- 触屏和触控板手势。

## 约束

- 只重排 `Winc.windows`，不改 `App` 的项目状态、`Shared` 结构和状态文件格式。
- 改动集中在 `view/workspace/projects.rs`、`app.rs`（`Drag` 变体与访问器）、`app/input/mouse`、`winc.rs`（`Request` 与新处理分支）。
- 文案不新增界面文字，无需改语言包。

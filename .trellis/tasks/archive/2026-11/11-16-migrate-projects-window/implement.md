# 执行计划：单窗口多项目迁入新窗口架构

前提：子任务 1 已合并回 `dev`，`dev` 上是上游的 `WindowKey`／`Request`／`Winc` 架构，项目列表功能不在。

分支：`git switch -c feat/projects-window dev`（一周内 `--no-ff` 回 `dev`）。

可用基线提交：`5b8d3ea`（merge base，项目列表最初版）、`c14f67e`（fork 最新，含拖拽排序）、`172b8aa`（拖拽排序）。

## C0 准备

```bash
git switch -c feat/projects-window dev
# 确认三处基线可读
git show c14f67e:crates/winc-app/src/view/workspace/projects.rs | head -1
git show c14f67e:crates/winc-app/src/winc.rs | rg -n "fn activate|fn close_project"
```

验收：分支创建成功，两条 `git show` 有输出。

## C1 恢复数据模型

| 文件 | 动作 |
| --- | --- |
| `crates/winc-app/src/winc.rs` | 加回 `Project { key, name, busy }`；`Shared` 加 `projects: RefCell<Vec<Project>>`；`Winc` 加 `active` 之外的 `activate`、`close_project`、`move_project`、`settle_projects`、`refresh_shared` 中的项目刷新 |
| `crates/winc-app/src/app.rs` | 加回 `project_closed: bool`；`projects_hits`／`projects_close`／`projects_keys`／`projects_scroll`／`projects_reveal` 五个字段及初始化 |
| `crates/winc-app/src/app/lifecycle/startup.rs` | 初始化上述字段（`deferred_close: None`、`project_closed: false`、五个空容器） |

取回参考：`git show c14f67e:crates/winc-app/src/winc.rs`。

验收：`cargo check -p winc-app` 通过；项目列表字段存在但无人使用。

## C2 窗口交接

新增 `WindowHandoff` 与三个方法（design §2）。

| 位置 | 内容 |
| --- | --- |
| `app/lifecycle/window.rs` | `pub(crate) struct WindowHandoff { handle: Arc<Window>, renderer: Renderer, display_link: ..., occluded: bool, frame: Option<WindowFrame>, controls: bool, dark: bool }`；`App::take_window`／`put_window`／`has_window` |
| `winc.rs` | `activate`：`take_window` 取当前窗口 → `put_window` 给目标 → 刷新 `active` 与 `Shared.projects` |
| `winc.rs` | `settle_projects`：收集 `project_closed` 的项目，窗口交给相邻项目，空列表时停在开始页 |
| `winc.rs` | `move_project`：只重排 `windows` 与 `Shared.projects`，不动窗口 |

验收：

```bash
cargo check -p winc-app
cargo test -p winc-app lifecycle::window
```

`#[cfg(debug_assertions)] fn assert_single_window` 在 `activate`、`settle_projects` 结尾调用。

## C3 Request 与启动流程

| 文件 | 动作 |
| --- | --- |
| `winc.rs` | `Request::NewWindow(Option<PathBuf>)` → `NewProject(Option<PathBuf>)`；处理改为：目录已开在某项目 → `activate`；否则 `App::new_project` + 插入 `windows` + `activate` |
| `app/lifecycle/startup.rs` | 拆分 `App::new_project`（不建窗口）与 `App::create_window`（建窗口 + 渲染器）；首个项目两步都走，后续只走第一步 |
| 全部调用点 | `Request::NewWindow` → `NewProject`（`rg -n "NewWindow" crates/` 清零） |

验收：`rg -n "NewWindow" crates/` 无输出；`cargo check -p winc-app`。

## C4 关闭流程

| 文件 | 动作 |
| --- | --- |
| `app.rs` | 加回 `deferred_close: Option<CloseRequest>`、`pending_ui: VecDeque<PendingUi>` |
| `app/lifecycle/closing.rs` | 加回 `request_close_project`、`close_project_now`、`resume_deferred_close`、`resume_pending_ui`、`can_handoff_for_prompt`（design §4.2） |
| `app/editor/format.rs` | 保留 fork 的作业号，接回 `deferred_close` 的格式化分支 |
| `app/workspace/files.rs` | 接回保存后的推迟关闭 |

核对上游新增的模态状态字段是否要进 `can_handoff_for_prompt`：

```bash
rg -n "pub\(crate\) (alert|notice|one_off|save_as_picker|form_dialog)" crates/winc-app/src/app.rs
```

逐个确认：对话框、选择器、确认框都返回 `false`。

验收：`cargo test -p winc-app lifecycle::closing`。

## C5 视图

| 动作 | 来源 |
| --- | --- |
| `crates/winc-app/src/view/workspace/projects.rs` 新建 | `git show c14f67e:` 原文件 |
| `view/workspace.rs` 加 `mod projects;` 与绘制调用 | 同上 |
| `layout.rs` 加 `Layout::projects`，`projects_width` 规则 | 同上 |
| `lifecycle/window.rs` 的 `place_traffic_lights` 加偏移 | 同上 |

验收：`cargo test -p winc-app snapshot::windows`。

## C6 输入

| 文件 | 动作 |
| --- | --- |
| `app/input/mouse/regions.rs` | projects 命中区域 |
| `app/input/mouse/click.rs` | 点击切换项目、点关闭按钮 |
| `app/input/mouse/drag.rs` | 拖拽排序的插入位置 |
| `app/input/context_menu/{menus,present}.rs` | 项目右键菜单（若 fork 有） |
| `keymap/action.rs` | 项目切换与前一个/后一个项目命令 |

取回参考：`git show c14f67e:crates/winc-app/src/app/input/mouse/drag.rs`。

验收：`rg -n "projects_hits|projects_close|projects_keys" crates/winc-app/src/app/input/` 有命中（说明已接线）。

## C7 文案

`crates/winc-i18n/locales/{en-US,zh-CN}/workspace.ftl` 与 `menu.ftl` 补项目列表相关条目，两语言键集合一致：

```bash
rg -o '^[a-z0-9-]+(?= =)' crates/winc-i18n/locales/en-US/workspace.ftl | sort > /tmp/en.keys
rg -o '^[a-z0-9-]+(?= =)' crates/winc-i18n/locales/zh-CN/workspace.ftl | sort > /tmp/zh.keys
diff /tmp/en.keys /tmp/zh.keys
```

验收：`diff` 无输出。

## C8 测试

| 文件 | 用例 |
| --- | --- |
| `app/snapshot/windows.rs` | 单项目无列表 / 多项目有列表 / 切换后标签与文档不变 / 最后一个项目关掉停在开始页 |
| `app/lifecycle/closing/tests.rs` | 关闭有未保存文档的项目 / 保存进行中再关闭并入 `deferred_close` / 退出优先于关闭项目 |
| `app/lifecycle/events/tests.rs` | 退出进行中 `NewProject` 被拒 |
| `app/lifecycle/window/tests.rs` | 移交保留句柄与渲染器 / 模态项目拒绝接收 |
| `app/snapshot/terminal/editor.rs` | 终端连接随项目保留 |

基线在 `c14f67e`，按新字段重写测试辅助函数：

```bash
git show c14f67e:crates/winc-app/src/app/snapshot/windows.rs
```

验收：`cargo test -p winc-app` 全绿。

## C9 门禁与合回

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast
```

已知基线失败只有 `winc-pty --test windows` 的伪控制台用例。

手工验证三条：

1. 开两个目录（含 worktree）→ 一个窗口、左侧两项 → 点第二项 → 标签、终端、焦点跟着换。
2. 第一项有未保存文档 → 点第二项 → 跳回第一项并弹出保存提示。
3. 关掉一项 → 列表剩一项、宽度为 0 → 再关掉 → 停在开始页。

合回：

```bash
git switch dev && git merge --no-ff feat/projects-window
git push origin dev
```

提交说明列触碰的上游高频文件：`winc.rs`、`app.rs`、`lifecycle/window.rs`、`lifecycle/closing.rs`、`layout.rs`、`input/mouse/*`。

## 回滚点

| 阶段 | 回滚 |
| --- | --- |
| C1–C8 | `git checkout -- .` 或丢掉分支 |
| C9 门禁失败 | 留在分支修复，不回 `dev` |
| 合回后发现问题 | `git revert -m 1 <merge>` |

## 顺序与依赖

C1 → C2 → C3 是主链，C2 卡住则 C3 无意义。C4 可与 C5 并行，但都依赖 C2。C6 依赖 C5（命中区域要与绘制矩形一致）。C8 依赖 C4–C6。

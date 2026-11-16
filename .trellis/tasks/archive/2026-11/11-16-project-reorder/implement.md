# 实现计划：项目列表拖拽排序

## 前提

- 落点用「间隙序号」表示：`0` 表示第一个条目之前，`len` 表示最后一个条目之后。
- 最终下标由间隙序号换算：`index = if gap > from { gap - 1 } else { gap }`；`from` 是按下时的条目序号。
- 拖动全程只请求一次 `Request::MoveProject`，拖动中的显示状态留在 `App` 里。

## 步骤

1. `crates/winc-app/src/app.rs`
   - 在 `Drag`（`:597`）加变体：
     ```rust
     /// 拖动项目列表中的条目：按下处的鼠标纵坐标、条目序号，以及当前落点的间隙序号。
     Project {
         key: crate::winc::WindowKey,
         start_y: f32,
         from: usize,
         drop: Option<usize>,
     },
     ```
   - 加访问器，供 `view/workspace/projects.rs` 与 `update_cursor_icon` 使用：
     ```rust
     pub(crate) fn dragged_project(&self) -> Option<(crate::winc::WindowKey, usize)> {
         match self.drag {
             Drag::Project { key, drop: Some(drop), .. } => Some((key, drop)),
             _ => None,
         }
     }
     ```
   - 拖动阈值复用标签拖动的 4 逻辑像素；在 `app.rs` 或 `projects.rs` 定义 `PROJECT_DRAG_THRESHOLD`，`tab_drag.rs` 的私有常量保持不变。

2. `crates/winc-app/src/app/input/mouse/click.rs:53-64`
   - `Hover::Project(key)`：不再立即 `Request::Focus`，改为记录 `Drag::Project { key, start_y: y, from, drop: None }`，`from` 取 `projects_hits` 中该 key 的序号；`set_hover` 与重绘照旧。
   - `Hover::ProjectClose(key)` 保持按下即请求关闭。

3. `crates/winc-app/src/app/input/mouse/drag.rs`（`on_mouse_move`，`:6-120`）
   - 在拖动分发前处理 `Drag::Project`：纵向移动未超过阈值时直接返回；超过后按列表几何算落点：
     `top = layout.projects.y + PROJECT_PAD - projects_scroll`，`gap = ((y - top) / (PROJECT_ROW + PROJECT_GAP)).round()`，夹到 `0..=len`。
   - 落点等于 `from` 或 `from + 1` 时视为没有移动，`drop = None`。
   - `drop` 变化时 `request_redraw`。
   - 拖动期间不更新悬停目标（与标签拖动一致）。

4. `crates/winc-app/src/app/input/mouse.rs`
   - `on_mouse_up`（`:93`）在清空拖动前处理 `Drag::Project`：
     - `drop == Some(gap)`：`self.shared.request(Request::MoveProject(key, index))`，`index` 按前提换算并夹到 `0..len-1`；
     - `drop == None`：`self.shared.request(Request::Focus(key))`。
   - 结束后清空拖动并重绘。
   - 键盘 Esc（`crates/winc-app/src/app/input/keyboard.rs:53`）在取消文件树拖动之后取消项目拖动：`drag = Drag::None`，重绘。
   - `update_cursor_icon`（`crates/winc-app/src/app/input/mouse/target.rs:194`）把 `dragged_project().is_some()` 并入拖动状态，显示抓取光标。

5. `crates/winc-app/src/view/workspace/projects.rs`
   - `draw_projects` 末尾按 `app.dragged_project()` 画落点线：`y = area.y + PROJECT_PAD - projects_scroll + gap * (PROJECT_ROW + PROJECT_GAP) - PROJECT_GAP / 2.0`，高 2 像素，`p.accent` 色，左右留 `PAD / 2.0`，用 `rect_clipped` 裁到 `area`。
   - 被拖动的条目文字用 `p.text_dim`，与可落点区分。

6. `crates/winc-app/src/winc.rs`
   - `Request` 加 `MoveProject(WindowKey, usize)`，文档注明「把项目 `key` 移到下标 `index`（重排后列表中的位置）」。
   - `after_event`（`:841-852`）加分支调用 `self.move_project(key, index)`：
     ```rust
     fn move_project(&mut self, key: WindowKey, index: usize) {
         let Some(from) = self.windows.iter().position(|slot| slot.key == key) else { return };
         let slot = self.windows.remove(from);
         let index = index.min(self.windows.len());
         self.windows.insert(index, slot);
     }
     ```
   - 顺序变化由下一轮 `settle()` 的 `refresh_shared()` 反映到 `shared.projects`，并让当前项目重绘；`active` 不变。

7. 测试（`crates/winc-app/src/app/snapshot/windows.rs`、`crates/winc-app/src/winc/tests.rs`）
   - `crates/winc-app/src/winc/tests.rs` 加 `move_project_for_test(key, index)` 包装。
   - 快照测试：
     - 三个项目，按下第三个条目、移动到最上方、松开：`requests` 含 `Request::MoveProject(c, 0)`；
     - `move_project_for_test` 后 `winc.keys()` 为 `[c, a, b]`，`shared.projects()` 的 key 顺序一致；
     - 只按下松开：`requests == [Request::Focus(b)]`（原有测试保持通过）；
     - 落点原位（`from`、`from + 1`）时松开不发 `MoveProject`，改发 `Focus`；
     - 拖到列表外：落点夹到 `0` 和 `len`。
   - 断言 `app.dragged_project()` 在超过阈值后为 `Some((key, gap))`，松开后为 `None`。

## 验证命令

```bash
cargo fmt --all
cargo clippy -p winc-app --all-targets
cargo test -p winc-app
```

Git 相关测试需要 `PATH` 含 Git 的 `cmd` 目录：

```bash
PATH="/c/Program Files/Git/cmd:$PATH" cargo test -p winc-app
```

## 回滚点

- 步骤 1-4 只增加拖动路径，点击语义在步骤 4 之前仍是按下即切换；若拖动不稳定，先回滚步骤 5 的绘制，再回滚 1-4。
- 步骤 6 是独立的请求分支，可单独回滚而不影响点击。

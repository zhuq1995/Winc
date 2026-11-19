//! 单窗口下的项目列表：所有项目共用一个系统窗口，最左侧的列表切换当前项目。
//!
//! 这里的用例都在离屏界面上跑：没有真实窗口，切项目时窗口的移交是空操作，可验证的部分是
//! 列表的宽度、条目、命中区域，以及关闭项目后留下的目录和项目顺序。

use winit::event::TouchPhase;
use winit::keyboard::{Key, NamedKey};

use super::*;

use crate::app::git::form::FormKind;
use crate::layout::{DialogPart, PROJECT_GAP, PROJECT_ROW};
use crate::winc::{Request, Winc, WindowKey};

/// 开着一个项目时列表不占位置，开着两个时占 `PROJECTS_WIDTH`。
#[test]
fn the_project_list_only_takes_room_with_more_than_one_project() {
    let dir = sample_folder("projects-width");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    winc.settle_for_test();
    assert_eq!(winc.app(a).projects_width(), 0.0);
    assert_eq!(winc.app(a).layout.projects.w, 0.0);

    let b = winc.add_detached(blank_app(false));
    winc.settle_for_test();
    assert_eq!(winc.keys(), [a, b]);
    assert_eq!(winc.app(a).projects_width(), crate::theme::PROJECTS_WIDTH);
}

/// 列表按项目逐个画一行，并记下它们的命中区域。
#[test]
fn the_list_draws_a_row_for_every_project() {
    let dir = sample_folder("projects-rows");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    let b = winc.add_detached(app_with(&dir.join("short.rs"), false));
    winc.settle_for_test();

    let app = winc.app(a);
    paint(app);

    assert_eq!(app.layout.projects.w, crate::theme::PROJECTS_WIDTH);
    assert_eq!(app.projects_keys, [a, b]);
    assert_eq!(app.projects_hits.len(), 2);
    assert!(app.projects_hits[0].0.bottom() <= app.projects_hits[1].0.y);

    // 关闭按钮只在鼠标停在该项目上时出现。
    let (row, _) = app.projects_hits[0];
    app.on_mouse_move(row.x + 20.0, row.y + row.h / 2.0);
    paint(app);
    assert_eq!(app.projects_close.len(), 1);
    assert_eq!(app.projects_close[0].1, a);
}

/// 点击列表中的另一项切换当前项目。
#[test]
fn clicking_a_project_switches_to_it() {
    let dir = sample_folder("projects-click");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    let b = winc.add_detached(blank_app(false));
    winc.settle_for_test();

    let app = winc.app(a);
    paint(app);
    let (rect, key) = app.projects_hits[1];
    assert_eq!(key, b);
    let (x, y) = (rect.x + 20.0, rect.y + rect.h / 2.0);
    app.pointer.cursor_inside = true;
    click(app, (x, y));

    let (requests, exit) = winc.settle_for_test();
    assert!(!exit);
    assert!(requests.contains(&Request::Focus(b)), "{requests:?}");
}

/// 拖到列表中的另一处再松开，把项目移到那里。
#[test]
fn dragging_a_project_moves_it() {
    let dir = sample_folder("projects-drag");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    let b = winc.add_detached(app_with(&dir.join("short.rs"), false));
    let c = winc.add_detached(blank_app(false));
    winc.settle_for_test();

    let app = winc.app(a);
    paint(app);
    let (first, _) = app.projects_hits[0];
    let third = app.projects_hits[2].0;
    let start = (first.x + 20.0, first.y + first.h / 2.0);
    app.pointer.cursor_inside = true;
    app.on_mouse_move(start.0, start.1);
    app.on_mouse_down(MouseButton::Left);
    app.on_mouse_move(start.0, first.y + crate::layout::PROJECT_ROW + 8.0);
    app.on_mouse_move(
        start.0,
        third.y + crate::layout::PROJECT_ROW + crate::layout::PROJECT_GAP + 4.0,
    );
    assert_eq!(app.dragged_project(), Some((a, 3)));
    app.on_mouse_up(MouseButton::Left);

    let (requests, _) = winc.settle_for_test();
    assert!(
        requests.contains(&Request::MoveProject(a, 2)),
        "{requests:?}"
    );
    winc.move_project_for_test(a, 2);
    assert_eq!(winc.keys(), [b, c, a]);
}

fn winc_with_projects(count: usize) -> (Winc, WindowKey) {
    let mut winc = Winc::new(None, None);
    let first = winc.add_detached(blank_app(false));
    for _ in 1..count {
        winc.add_detached(App::new(
            Editor::default(),
            None,
            Bench::from_env(),
            Config::default(),
        ));
    }
    winc.settle_for_test();
    (winc, first)
}

fn center(rect: Rect) -> (f32, f32) {
    (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
}

fn press_first_project(app: &mut App) -> (f32, f32) {
    let (x, y) = center(app.projects_hits[0].0);
    app.pointer.cursor_inside = true;
    app.on_mouse_move(x, y);
    app.on_mouse_down(MouseButton::Left);
    (x, y)
}

#[test]
fn project_rows_and_close_buttons_do_not_respond_under_modals() {
    let (mut winc, first) = winc_with_projects(5);
    let app = winc.app(first);
    app.frame.renderer =
        Some(Renderer::offscreen(SIZE.0, 180.0, SCALE, app.palette.frame_bg, None, None).unwrap());
    paint(app);
    let row = center(app.projects_hits[1].0);
    app.pointer.cursor_inside = true;
    app.on_mouse_move(row.0, row.1);
    paint(app);
    let close = center(app.projects_close[0].0);

    let scroll = |app: &mut App, point: (f32, f32)| {
        app.on_mouse_move(point.0, point.1);
        app.on_wheel_event(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -32.0 * f64::from(SCALE))),
            TouchPhase::Started,
        );
        assert_eq!(app.projects_scroll, 0.0);
    };
    for point in [row, close] {
        app.open_settings();
        assert_eq!(app.hover_at(point.0, point.1), Hover::None);
        scroll(app, point);
        click(app, point);
        assert!(!app.settings.open);

        app.show_form_dialog(FormKind::Worktree, TextField::default(), DialogPart::Branch);
        assert_eq!(app.hover_at(point.0, point.1), Hover::None);
        scroll(app, point);
        click(app, point);
        assert!(app.overlays.form_dialog.is_none());
    }

    let (requests, _) = winc.settle_for_test();
    assert!(requests.is_empty(), "{requests:?}");
}

#[test]
fn clicking_a_project_dismisses_the_picker_without_switching() {
    let (mut winc, first) = winc_with_projects(2);
    let app = winc.app(first);
    paint(app);
    let point = center(app.projects_hits[1].0);
    app.overlays.picker.open = true;
    app.focus = Focus::Picker;
    app.pointer.cursor_inside = true;
    click(app, point);
    assert!(!app.overlays.picker.open);

    let (requests, _) = winc.settle_for_test();
    assert!(requests.is_empty(), "{requests:?}");
}

#[test]
fn escape_cancels_a_pressed_or_dragged_project() {
    let (mut winc, first) = winc_with_projects(2);
    let app = winc.app(first);
    paint(app);
    let end_y = app.projects_hits[1].0.bottom() - 1.0;
    for dragging in [false, true] {
        let (x, _) = press_first_project(app);
        if dragging {
            app.on_mouse_move(x, end_y);
            assert_eq!(app.dragged_project(), Some((first, 2)));
        }
        assert!(app.cancel_drag_on_escape(&Key::Named(NamedKey::Escape)));
        app.on_mouse_up(MouseButton::Left);
    }

    let (requests, _) = winc.settle_for_test();
    assert!(requests.is_empty(), "{requests:?}");
}

#[test]
fn releasing_a_project_outside_the_list_discards_the_drop() {
    let (mut winc, first) = winc_with_projects(2);
    let app = winc.app(first);
    paint(app);
    let end_y = app.projects_hits[1].0.bottom() - 1.0;
    for leave_window in [false, true] {
        let (x, _) = press_first_project(app);
        app.on_mouse_move(x, end_y);
        assert_eq!(app.dragged_project(), Some((first, 2)));
        if leave_window {
            app.cursor_left();
        } else {
            app.on_mouse_move(app.layout.projects.right(), end_y);
            assert_eq!(app.dragged_project(), None);
        }
        app.on_mouse_up(MouseButton::Left);
    }

    let (requests, _) = winc.settle_for_test();
    assert!(requests.is_empty(), "{requests:?}");
}

#[test]
fn releasing_a_project_after_its_row_changes_discards_the_drop() {
    for repaint in [false, true] {
        let (mut winc, first) = winc_with_projects(2);
        let app = winc.app(first);
        paint(app);
        let (x, _) = press_first_project(app);
        app.on_mouse_move(x, app.projects_hits[1].0.bottom() - 1.0);
        assert_eq!(app.dragged_project(), Some((first, 2)));
        winc.move_project_for_test(first, 1);

        let app = winc.app(first);
        if repaint {
            paint(app);
        }
        app.on_mouse_up(MouseButton::Left);
        let (requests, _) = winc.settle_for_test();
        assert!(requests.is_empty(), "{requests:?}");
    }
}

#[test]
fn scrolling_during_a_project_drag_updates_the_drop() {
    let (mut winc, first) = winc_with_projects(5);
    let app = winc.app(first);
    app.frame.renderer =
        Some(Renderer::offscreen(SIZE.0, 180.0, SCALE, app.palette.frame_bg, None, None).unwrap());
    paint(app);
    let (x, _) = press_first_project(app);
    app.on_mouse_move(x, app.projects_hits[1].0.bottom() - 1.0);
    assert_eq!(app.dragged_project(), Some((first, 2)));

    let stride = PROJECT_ROW + PROJECT_GAP;
    app.on_wheel_event(
        MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, f64::from(-stride * SCALE))),
        TouchPhase::Started,
    );
    assert_eq!(app.projects_scroll, stride);
    assert!(app.projects_hits.is_empty());
    assert_eq!(app.dragged_project(), Some((first, 3)));
    app.on_mouse_up(MouseButton::Left);

    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::MoveProject(first, 2)]);
}

#[test]
fn narrowing_the_project_viewport_ignores_old_hit_areas() {
    let (mut winc, first) = winc_with_projects(2);
    let second = winc.keys()[1];
    let app = winc.app(first);
    paint(app);
    let (x, _) = press_first_project(app);
    let end_y = app.projects_hits[1].0.bottom() - 1.0;
    app.on_mouse_move(x, end_y);
    assert_eq!(app.dragged_project(), Some((first, 2)));

    app.frame.renderer =
        Some(Renderer::offscreen(106.0, SIZE.1, SCALE, app.palette.frame_bg, None, None).unwrap());
    app.relayout();
    assert!(!app.layout.projects.contains(x, end_y));
    app.on_mouse_up(MouseButton::Left);
    click(app, (x, end_y));

    paint(app);
    let point = center(app.projects_hits[1].0);
    click(app, point);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::Focus(second)]);
}

/// 关掉一个项目后列表里只剩其他项目，它们的目录不变。
#[test]
fn closing_a_project_leaves_the_others_alone() {
    let dir = sample_folder("projects-close");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    let b = winc.add_detached(app_with(&dir.join("short.rs"), false));
    winc.settle_for_test();

    winc.set_active(a);
    winc.app(b).request_close_project();
    winc.settle_for_test();

    assert_eq!(winc.keys(), [a]);
    assert_eq!(winc.app(a).editor.root(), Some(dir.join("src").as_path()));
}

/// 最后一个项目关掉后停在开始页，列表里只剩它一个，不占位置。
#[test]
fn closing_the_last_project_leaves_it_on_the_start_page() {
    let dir = sample_folder("projects-last");
    let mut winc = crate::winc::Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    winc.settle_for_test();

    winc.app(a).request_close_project();
    winc.settle_for_test();

    assert_eq!(winc.keys(), [a]);
    assert!(winc.app(a).editor.root().is_none());
    assert_eq!(winc.app(a).projects_width(), 0.0);
}

/// 不同位置下的同名目录：各自加上级目录区分，否则列表里两行看起来一样。
#[test]
fn projects_with_the_same_folder_name_show_their_parent() {
    let a_dir = sample_folder("projects-names/a/proj");
    let b_dir = sample_folder("projects-names/b/proj");
    let mut winc = crate::winc::Winc::new(None, None);
    winc.add_detached(app_with(&a_dir.join("short.rs"), false));
    winc.add_detached(app_with(&b_dir.join("short.rs"), false));
    winc.settle_for_test();

    assert_eq!(
        winc.project_names(),
        [Some("a/proj".to_owned()), Some("b/proj".to_owned())]
    );
}

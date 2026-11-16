//! 单窗口下的项目列表：所有项目共用一个系统窗口，最左侧的列表切换当前项目。
//!
//! 这里的用例都在离屏界面上跑：没有真实窗口，切项目时窗口的移交是空操作，可验证的部分是
//! 列表的宽度、条目、命中区域，以及关闭项目后留下的目录和项目顺序。

use super::*;

use crate::winc::Request;

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
    app.on_mouse_move(x, y);
    click(app, (x, y));
    app.on_mouse_up(MouseButton::Left);

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

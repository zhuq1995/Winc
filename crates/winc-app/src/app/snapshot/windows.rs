//! 单进程多项目（`crate::winc`）的测试：关闭单个项目、⌘Q 退出全部、切换到开着目录的项目、
//! 项目之间同步设置与被剪切的文件。一个进程只有一个系统窗口，项目之间靠窗口交接切换。
//!
//! 测试没有事件循环，每步之后手动执行一次应用层收尾
//! （`settle_for_test`）。

use super::*;
use crate::winc::{Request, Winc};

/// 两个窗口：第一个开着 `name` 目录里的 `src/main.rs`，第二个在开始页。
fn two_windows(
    name: &str,
) -> (
    Winc,
    crate::winc::WindowKey,
    crate::winc::WindowKey,
    PathBuf,
) {
    let dir = search_folder(name);
    let mut winc = Winc::new(None, None);
    let a = winc.add_detached(app_with(&dir.join("src/main.rs"), false));
    let b = winc.add_detached(blank_app(false));
    (winc, a, b, dir)
}

#[test]
fn closing_one_project_keeps_the_other_and_the_last_one_shows_the_start_page() {
    let (mut winc, a, b, dir) = two_windows("windows-close");
    winc.app(b).request_close_project();
    winc.settle_for_test();
    assert_eq!(winc.keys(), [a], "关掉的项目从列表里去掉");

    // 最后一个项目：留在开始页上，窗口仍然显示，不退出。
    winc.app(a).request_close_project();
    let (_, exit) = winc.settle_for_test();
    assert_eq!(winc.keys(), [a]);
    assert!(!exit && !winc.app(a).quit_requested);
    assert!(winc.app(a).editor.workspace.is_none() && winc.app(a).editor.documents.is_empty());
    assert!(!winc.app(a).project_closed, "最后一个项目留在开始页上");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_project_without_a_folder_asks_about_unsaved_files() {
    let (mut winc, a, b, dir) = two_windows("windows-close-untitled");
    let root = super::workspace::session::backup_root("windows-close-untitled");
    for key in [a, b] {
        winc.app(key).backup.root = Some(root.clone());
    }
    // 没开目录的项目中有未保存的笔记：备份没有之后恢复的时机，关闭前必须询问。
    winc.app(b).new_file();
    winc.app(b).run(Command::Insert("笔记".into()));
    winc.app(b).request_close_project();
    assert!(winc.app(b).save_prompt.is_some() && !winc.app(b).project_closed);
    winc.app(b).answer_save_prompt(SaveChoice::Cancel);
    winc.settle_for_test();
    assert_eq!(winc.keys(), [a, b], "取消了就留着");
    assert!(!winc.app(b).project_closed);

    // 开着目录的项目仍然不询问：内容写入备份，以后打开该目录时恢复。
    winc.app(a).run(Command::Insert(
        "// 还没存
"
        .into(),
    ));
    winc.app(a).request_close_project();
    assert!(winc.app(a).save_prompt.is_none());
    winc.settle_for_test();
    assert_eq!(
        winc.keys(),
        [b],
        "关掉的项目从列表里去掉，剩下停在开始页上的那个"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_project_list_shows_every_project_and_switches_on_click() {
    let (mut winc, a, b, dir) = two_windows("windows-projects");
    winc.settle_for_test();
    let app = winc.app(a);
    paint(app);

    // 两个项目：左边一列列出它们，每个条目记下可点击区域。
    let projects: Vec<Option<String>> = (app.shared.projects().iter())
        .map(|project| project.name.clone())
        .collect();
    assert_eq!(projects, [Some("src".to_owned()), None]);
    assert!(app.layout.projects.w > 0.0, "开着多个项目时显示列表");
    assert_eq!(app.projects_hits.len(), 2);
    let (row, key) = app.projects_hits[1];
    assert_eq!(key, b);

    // 点条目：请应用层切到那个项目。
    click(app, (row.x + row.w / 2.0, row.y + row.h / 2.0));
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::Focus(b)]);
    assert_eq!(winc.active_for_test(), Some(a), "切换由应用层执行");

    // 悬停在条目上：右端出现关闭按钮，点它请应用层关掉那个项目。
    let app = winc.app(b);
    paint(app);
    let (row, _) = app.projects_hits[1];
    app.on_mouse_move(row.x + row.w / 2.0, row.y + row.h / 2.0);
    assert_eq!(app.hover, Hover::Project(b));
    paint(app);
    let (close, key) = app.projects_close[0];
    assert_eq!(key, b);
    click(app, (close.x + close.w / 2.0, close.y + close.h / 2.0));
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::CloseProject(b)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn switching_projects_keeps_each_ones_editor_and_terminal_state() {
    let (mut winc, a, b, dir) = two_windows("windows-switch");
    winc.settle_for_test();

    // 第一个项目里改一处没保存的内容，切到第二个项目再切回来：内容还在，仍是未保存。
    winc.app(a).run(Command::Insert(
        "// 还没存
"
        .into(),
    ));
    let saved = winc.app(a).editor.active().unwrap().id;
    winc.activate_for_test(b);
    assert_eq!(winc.active_for_test(), Some(b));
    assert_eq!(winc.app(b).editor.root(), None);
    assert!(
        winc.app(a).editor.root().is_some(),
        "原来的项目还开着那个目录"
    );

    winc.activate_for_test(a);
    assert_eq!(winc.active_for_test(), Some(a));
    let doc = winc.app(a).editor.active().unwrap();
    assert_eq!(doc.id, saved);
    assert!(doc.is_dirty());
    assert!(doc.text().rope().unwrap().to_string().starts_with(
        "// 还没存
"
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_opened_by_a_project_of_this_window_switches_to_it() {
    let (mut winc, a, b, dir) = two_windows("windows-reopen");
    winc.settle_for_test();
    let root = winc.app(b).editor.root().map(Path::to_path_buf);
    assert_eq!(root, None);
    winc.activate_for_test(b);
    // 第二个项目打开第一个项目已开着的目录：不重复打开，切到第一个项目。
    let src = winc_config::files::canonicalize(dir.join("src")).unwrap();
    winc.app(b).open_folder(&src);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::OpenFolder(src)]);
    let _ = a;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_one_window_quits_all_of_them() {
    let (mut winc, a, b, dir) = two_windows("windows-quit");
    winc.set_active(a);
    winc.app(a).request_quit();
    let (_, exit) = winc.settle_for_test();
    assert!(winc.app(b).quit_requested, "其他窗口也请它退出");
    assert!(exit, "都退了才结束事件循环");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelling_in_one_window_calls_the_whole_quit_off() {
    let (mut winc, a, b, dir) = two_windows("windows-quit-cancel");
    // 第二个窗口有不能热退出的修改：退出时要问。
    winc.app(b).config.hot_exit = Some(false);
    winc.app(b).new_file();
    winc.app(b).run(Command::Insert("x".into()));
    winc.set_active(b);

    winc.app(a).request_quit();
    let (_, exit) = winc.settle_for_test();
    assert!(!exit && winc.app(b).save_prompt.is_some(), "第二个窗口在问");
    assert!(winc.app(a).quit_requested);

    winc.app(b).answer_save_prompt(SaveChoice::Cancel);
    let (_, exit) = winc.settle_for_test();
    assert!(!exit);
    assert!(
        !winc.app(a).quit_requested,
        "取消了：已经同意退出的窗口照常开着"
    );
    assert!(!winc.app(b).quit_requested);

    // 再退一次，这次不保存：都退了。
    winc.app(a).request_quit();
    winc.settle_for_test();
    winc.app(b).answer_save_prompt(SaveChoice::Discard);
    let (_, exit) = winc.settle_for_test();
    assert!(exit);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_folder_that_another_window_has_open_switches_to_that_window() {
    let (mut winc, a, b, dir) = two_windows("windows-focus");
    // 第一个窗口开着的是 `src`（打开文件时以它所在的目录为项目）。
    winc.app(b).switch_folder(&dir.join("src"));
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::Focus(a)]);
    assert!(winc.app(b).editor.root().is_none(), "不在这里再开一份");

    // 提示框里选了切过去（移除开在另一个窗口里的工作树时）：同样只切过去；那个窗口已经关了时
    // 什么也不做。
    let src = winc_config::files::canonicalize(dir.join("src")).unwrap();
    assert!(winc.app(b).focus_window_with(&src));
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::Focus(a)]);
    assert!(!winc.app(b).focus_window_with(&dir));
    let (requests, _) = winc.settle_for_test();
    assert!(requests.is_empty());

    // 新建窗口的请求交给应用层。
    winc.app(b).run_action(crate::keymap::Action::NewWindow);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::NewWindow(None)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_another_folder_asks_for_a_new_project() {
    let (mut winc, a, _, dir) = two_windows("windows-open-in");
    winc.settle_for_test();
    let root = winc_config::files::canonicalize(&dir).unwrap();
    let src = winc.app(a).editor.root().unwrap().to_path_buf();

    // 打开另一个目录：当前项目不变，请应用层新开一个项目。旧设置在配置里保留下来，不再由它
    // 决定打开方式。
    winc.app(a).config.open_folder_in_new_window = Some(false);
    winc.app(a).open_path(&dir);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::OpenFolder(root.clone())]);
    assert_eq!(winc.app(a).editor.root(), Some(src.as_path()));
    assert_eq!(winc.app(a).config.open_folder_in_new_window, Some(false));

    // 打开已经开着的目录也一样：由应用层切到那个项目。
    winc.app(a).open_path(&src);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(requests, [Request::OpenFolder(src.clone())]);
    assert_eq!(winc.app(a).editor.root(), Some(src.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropped_on_the_dock_icon_open_in_the_window_that_has_them_or_a_blank_one() {
    let (mut winc, a, b, dir) = two_windows("windows-dock");
    winc.settle_for_test();
    let src = winc_config::files::canonicalize(dir.join("src")).unwrap();
    let target = |path: &Path| crate::cli::Target {
        path: path.to_path_buf(),
        line: None,
        col: None,
    };

    // 开着它所在目录的窗口收下。
    assert_eq!(winc.window_for_test(&[target(&src.join("main.rs"))]), Ok(a));

    // 其他目录的文件：没有窗口开着该目录，第二个窗口停在开始页上，就在该窗口里打开文件
    // 所在的目录，不再新建窗口。
    let elsewhere = winc_config::files::canonicalize(&dir).unwrap();
    winc.set_active(a);
    assert_eq!(
        winc.window_for_test(&[target(&elsewhere.join("a.txt"))]),
        Ok(b)
    );
    assert_eq!(winc.app(b).editor.root(), Some(elsewhere.as_path()));
    winc.settle_for_test();

    // 两个窗口都开着目录：要新建窗口打开别的目录。
    let other = search_folder("windows-dock-other");
    let other = winc_config::files::canonicalize(&other).unwrap();
    assert_eq!(
        winc.window_for_test(&[target(&other)]),
        Err(Some(other.clone()))
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn settings_changed_in_one_window_reach_the_others() {
    let (mut winc, a, b, dir) = two_windows("windows-config");
    winc.app(a).step_font_size(Some(true));
    let size = winc.app(a).config.editor_size;
    assert!(size.is_some());
    winc.settle_for_test();
    assert_eq!(winc.app(b).config.editor_size, size);
    assert_eq!(winc.app(b).fonts.code, size.unwrap(), "字号也用上了");

    winc.app(b).settings_toggle(Setting::WordWrap);
    winc.settle_for_test();
    assert_eq!(winc.app(a).config.word_wrap, Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_written_back_keep_what_was_edited_in_the_file_meanwhile() {
    let (mut winc, a, b, dir) = two_windows("windows-config-edits");
    let path = dir.join("config.json");
    for key in [a, b] {
        winc.app(key).config_path = Some(path.clone());
    }
    // 程序运行期间，用户在 winc 之外手动修改了配置文件，之后在设置页中修改了另一项。
    std::fs::write(&path, r#"{"editor": {"size": 16}}"#).unwrap();
    winc.app(a).settings_toggle(Setting::WordWrap);
    winc.settle_for_test();

    let (config, _) = Config::load_from(&path);
    assert_eq!(
        (config.editor_size, config.word_wrap),
        (Some(16.0), Some(true))
    );
    // 手动修改的字号没有被覆盖，两个窗口都已应用。
    for key in [a, b] {
        assert_eq!(winc.app(key).config, config);
        assert_eq!(winc.app(key).fonts.code, 16.0);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_cut_in_one_window_are_moved_when_pasted_in_another() {
    let (mut winc, a, b, dir) = two_windows("windows-cut");
    let file = dir.join("a.txt");
    winc.app(a).cut_entries(std::slice::from_ref(&file));
    assert_eq!(*winc.app(b).shared.cut_files.borrow(), [file]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_go_to_the_innermost_window_that_contains_them() {
    let (mut winc, a, b, dir) = two_windows("windows-containing");
    winc.app(b).open_folder_now(&dir);
    winc.settle_for_test();
    let shared = std::rc::Rc::clone(&winc.app(a).shared);
    let src = winc_config::files::canonicalize(dir.join("src")).unwrap();
    assert_eq!(
        shared.window_containing(&src.join("main.rs")),
        Some(a),
        "最里面的目录"
    );
    assert_eq!(
        shared.window_containing(&src.with_file_name("a.txt")),
        Some(b)
    );
    assert_eq!(shared.window_containing(Path::new("/elsewhere/x")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_windows_are_remembered_at_quit_and_reopened_at_launch() {
    let first = search_folder("windows-restore-a");
    let second = search_folder("windows-restore-b");
    let state_path = first.with_file_name("windows-restore-state.json");
    let _ = std::fs::remove_file(&state_path);
    let template = crate::winc::Template {
        config_path: None,
        state_path: Some(state_path.clone()),
        backup_root: None,
    };
    let mut winc = Winc::new(None, Some(template.clone()));
    let a = winc.add_detached(app_with(&first, false));
    let b = winc.add_detached(app_with(&second, false));
    winc.set_active(a);
    winc.remember_windows_for_test();
    let root = |winc: &mut Winc, key| winc.app(key).editor.root().map(Path::to_path_buf);
    let others = State::load_from(&state_path).others;
    assert_eq!(
        others,
        [root(&mut winc, b).unwrap()],
        "当前窗口的由它自己记在 `last`"
    );

    // 下次启动：第一个窗口在开始页上时，第一个目录开在它里面，不多留一个空窗口。
    let mut winc = Winc::new(None, Some(template));
    let a = winc.add_detached(blank_app(false));
    winc.restore_windows(&others);
    assert_eq!(root(&mut winc, a), others.first().cloned());
    assert!(winc.settle_for_test().0.is_empty());

    // 第一个窗口开着别的目录：其余的各开一个窗口，最后切回第一个窗口。
    let mut winc = Winc::new(None, None);
    let a = winc.add_detached(app_with(&first, false));
    winc.restore_windows(&others);
    let (requests, _) = winc.settle_for_test();
    assert_eq!(
        requests,
        [Request::OpenFolder(others[0].clone()), Request::Focus(a)]
    );
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
    let _ = std::fs::remove_file(&state_path);
}

/// 在一个窗口剪切、到另一个窗口的目录里粘贴。
///
/// 回归：此前的实现在一个窗口剪切、到另一个窗口粘贴时失败，只能移动本窗口目录里的文件；
/// 文件移过去以后，剪切那边打开着的文档要跟到新位置。
#[test]
fn files_cut_in_one_window_move_into_another_windows_folder() {
    let a_dir = search_folder("windows-move-a");
    let b_dir = search_folder("windows-move-b");
    let notes = a_dir.join("notes.txt");
    std::fs::write(&notes, "笔记\n").unwrap();
    let mut winc = Winc::new(None, None);
    let a = winc.add_detached(app_with(&notes, false));
    let b = winc.add_detached(app_with(&b_dir.join("src/main.rs"), false));

    winc.app(a).cut_entries(std::slice::from_ref(&notes));
    winc.app(b).copied_files = vec![notes.clone()];
    let target = winc.app(b).editor.workspace.as_ref().unwrap().target_dir();
    winc.app(b).paste_into_tree();
    winc.settle_for_test();

    let moved = target.join("notes.txt");
    assert!(moved.exists() && !notes.exists());
    let doc = winc.app(a).editor.active().unwrap();
    assert_eq!(doc.path.as_deref(), Some(moved.as_path()));
    assert!(winc.app(b).shared.cut_files.borrow().is_empty(), "只移一次");
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}

/// 回归：退出时另一个窗口正在问「关标签要不要保存」，它答完以后整个退出被当作取消。
#[test]
fn a_window_busy_with_another_question_is_asked_again_after_it() {
    let (mut winc, a, b, dir) = two_windows("windows-quit-busy");
    winc.app(b).config.hot_exit = Some(false);
    winc.app(b).new_file();
    winc.app(b).run(Command::Insert("x".into()));
    let tab = winc.app(b).editor.active.unwrap();
    winc.app(b).close_tab(tab);
    assert!(winc.app(b).save_prompt.is_some(), "在问关标签要不要保存");

    winc.set_active(a);
    winc.app(a).request_quit();
    let (_, exit) = winc.settle_for_test();
    assert!(!exit && !winc.app(b).quit_requested, "先等它问完");

    // 关标签那件事答完：再请它退出，标签已关、没有要问的，整个退出完成。
    winc.app(b).answer_save_prompt(SaveChoice::Discard);
    let (_, exit) = winc.settle_for_test();
    assert!(exit && winc.app(b).quit_requested);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 开着热退出时，已写进备份的文档在退出时不再询问。
///
/// 回归：此前的实现只要有未保存的文档就要询问，macOS 上取消退出会让系统注销和关机随之中断。
/// 没写过备份的文档、关了热退出的文档仍然要问。
#[test]
fn unsaved_files_already_backed_up_do_not_need_asking_at_quit() {
    let dir = search_folder("windows-quit-backed-up");
    let root = super::workspace::session::backup_root("windows-quit-backed-up");
    let mut app = app_with(&dir.join("src/main.rs"), false);
    app.backup.root = Some(root.clone());
    assert!(!app.asks_before_quit());

    app.run(Command::Insert("// 还没存\n".into()));
    assert!(app.asks_before_quit(), "还没写过备份");
    super::workspace::session::back_up_now(&mut app);
    assert!(
        !app.asks_before_quit(),
        "已写进备份，退出时再按当前内容写一次"
    );
    // 退出时按当前内容再写一次：备份之后又输入的内容也在里面。
    app.run(Command::Insert("// 又改了\n".into()));
    assert!(!app.asks_before_quit());
    app.prepare_exit();
    let folder = app.editor.workspace.as_ref().unwrap().root.clone();
    let backups = winc_editor::backup::Backups::new(&root, Some(&folder)).read_all();
    let text = backups[0].text().unwrap();
    assert!(text.starts_with("// 还没存\n// 又改了\n"), "{text:?}");

    app.config.hot_exit = Some(false);
    assert!(app.asks_before_quit());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&root);
}

/// 应用绘制标题栏时（macOS 以外的平台）：顶栏右端是最小化、最大化、关闭三个按钮，标签栏
/// 让出该区域；窗口四边是改变大小的细边；点关闭按钮和系统标题栏上的关闭按钮一样关掉该窗口。
#[test]
fn a_drawn_title_bar_has_window_buttons_and_resize_edges() {
    use crate::app::WindowControl;
    use winit::window::ResizeDirection;

    let (mut winc, a, b, dir) = two_windows("windows-title-bar");
    let app = winc.app(a);
    paint(app);
    assert_eq!(app.layout.controls, Rect::default(), "默认用系统的标题栏");
    app.window_controls = true;
    app.relayout();
    paint(app);
    let controls = app.layout.controls;
    let window = app.layout.window;
    assert!(controls.w > 0.0 && controls.right() < window.right());
    assert!(controls.y >= 0.0 && controls.bottom() <= TOP_BAR_END);
    assert!(app.layout.tabs.right() <= controls.x, "标签栏让出窗口按钮");

    let rects = app.window_control_rects();
    let order: Vec<_> = rects.iter().map(|&(_, c)| c).collect();
    assert_eq!(order, WindowControl::ALL);
    for (rect, control) in rects {
        let (x, y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        assert_eq!(app.hover_at(x, y), Hover::WindowControl(control));
    }

    // 四边和四角：改变大小的方向和对应的指针；稍微往内就是窗口中的内容。
    let (w, h) = (window.w, window.h);
    for ((x, y), direction) in [
        ((1.0, 1.0), ResizeDirection::NorthWest),
        ((w - 1.0, h - 1.0), ResizeDirection::SouthEast),
        ((w / 2.0, 1.0), ResizeDirection::North),
        ((1.0, h / 2.0), ResizeDirection::West),
        ((w - 1.0, h / 2.0), ResizeDirection::East),
        ((w / 2.0, h - 1.0), ResizeDirection::South),
    ] {
        assert_eq!(app.hover_at(x, y), Hover::ResizeEdge(direction));
    }
    assert!(!matches!(app.hover_at(w / 2.0, 12.0), Hover::ResizeEdge(_)));
    app.on_mouse_move(w - 1.0, h / 2.0);
    assert_eq!(app.cursor_icon, CursorIcon::EResize);

    // 关闭按钮：和系统标题栏上的一样关闭窗口。窗口里放着所有项目：macOS 上隐藏，其他平台上
    // 就是退出，各项目逐个确认。
    let (close, _) = app.window_control_rects()[2];
    click(app, (close.x + close.w / 2.0, close.y + close.h / 2.0));
    let (_, exit) = winc.settle_for_test();
    assert_eq!(winc.keys(), [a, b]);
    if cfg!(target_os = "macos") {
        assert!(!exit && winc.app(a).window_hidden);
    } else {
        assert!(winc.app(a).quit_requested && winc.app(b).quit_requested && exit);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// 「使用系统标题栏」（macOS 以外的平台）：开关马上生效，顶栏右端的窗口按钮随之去掉或回来。
#[cfg(not(target_os = "macos"))]
#[test]
fn the_title_bar_setting_applies_at_once() {
    use crate::layout::Setting;

    let (mut winc, a, _, dir) = two_windows("windows-title-bar-setting");
    let app = winc.app(a);
    app.sync_title_bar();
    app.relayout();
    assert!(app.window_controls && app.layout.controls.w > 0.0);
    assert!(!app.switch_on(Setting::SystemTitleBar));
    app.settings_toggle(Setting::SystemTitleBar);
    assert_eq!(app.config.ui_native_title_bar, Some(true));
    assert!(!app.window_controls);
    assert_eq!(app.layout.controls, Rect::default());
    app.settings_toggle(Setting::SystemTitleBar);
    assert!(app.window_controls && app.layout.controls.w > 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "截图，手动运行"]
fn snapshot_project_list() {
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        let dir = search_folder(&format!("projects-shot-{theme}"));
        let other = sample_folder(&format!("projects-shot-other-{theme}"));
        let mut winc = Winc::new(None, None);
        let a = winc.add_detached(app_with(&dir.join("src/main.rs"), dark));
        let b = winc.add_detached(app_with(&other.join("a.txt"), dark));
        winc.settle_for_test();
        let app = winc.app(a);
        app.on_mouse_move(900.0, 500.0);
        shoot(app, &format!("projects-{theme}"));

        // 鼠标停在第二个条目上：右端出现关闭按钮，条目铺一层底色。
        let (row, _) = app.projects_hits[1];
        let (x, y) = (row.x + row.w / 2.0, row.y + row.h / 2.0);
        app.on_mouse_move(x, y);
        shoot(app, &format!("projects-hover-{theme}"));

        // 侧栏收起时列表仍在最左侧。
        app.set_sidebar(false);
        shoot(app, &format!("projects-narrow-{theme}"));
        let _ = b;
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }
}

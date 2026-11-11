//! 关闭流程的测试：提示框按钮的序号与保存选择的对应。

use super::*;

#[test]
fn buttons_map_to_choices_in_macos_order() {
    assert_eq!(SaveChoice::from_index(0), SaveChoice::Save);
    assert_eq!(SaveChoice::from_index(1), SaveChoice::Cancel);
    assert_eq!(SaveChoice::from_index(2), SaveChoice::Discard);
    assert_eq!(SaveChoice::from_index(9), SaveChoice::Cancel);
}

#[test]
fn hiding_a_dirty_window_keeps_documents_and_explicit_quit_still_prompts() {
    let mut app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    app.config.hot_exit = Some(false);
    app.editor.new_untitled();
    app.editor
        .active_mut()
        .unwrap()
        .execute(Command::Insert("draft".into()), 1, 4);
    let mut terminal = crate::app::terminal::TermTab::new(1, Default::default());
    terminal.process = "vim".into();
    app.terminal.tabs.push(terminal);
    app.terminal.open = true;
    app.settings.open = true;
    assert!(app.terminal_busy());

    if cfg!(target_os = "macos") {
        app.request_close_window();
    } else {
        app.request_close(CloseRequest::CloseWindow);
    }
    assert!(app.window.hidden);
    assert!(!app.window.focused);
    assert!(
        !app.closing.quit_requested
            && app.closing.prompt.is_none()
            && app.closing.confirming.is_none()
    );
    assert!(app.editor.active().unwrap().is_dirty());
    assert_eq!(app.terminal.tabs.len(), 1);
    assert!(app.terminal_busy() && app.settings.open);

    app.request_quit();
    assert_eq!(
        app.closing.prompt.as_ref().unwrap().request,
        CloseRequest::Quit
    );
    app.answer_save_prompt(SaveChoice::Cancel);
    assert!(app.window.hidden && !app.closing.quit_requested);
    assert!(app.editor.active().unwrap().is_dirty());
}

#[test]
fn hiding_records_untitled_backup_in_the_workspace_session() {
    let root = std::env::temp_dir().join(format!("winc-hide-backup-{}", std::process::id()));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut editor = Editor::default();
    editor.open_folder(&workspace).unwrap();
    editor.new_untitled();
    editor
        .active_mut()
        .unwrap()
        .execute(Command::Insert("draft".into()), 1, 4);
    let mut app = App::new(editor, None, crate::bench::Bench::off(), Config::default());
    app.backup.root = Some(root.join("backups"));

    app.request_close(CloseRequest::CloseWindow);
    let session = app.state.session_for(&workspace).unwrap();
    assert!(matches!(
        session.tabs.as_slice(),
        [winc_config::Tab::Untitled(_)]
    ));
    let backups =
        winc_editor::backup::Backups::new(app.backup.root.as_ref().unwrap(), Some(&workspace));
    assert_eq!(backups.read_all().len(), 1);
    assert!(app.window.hidden && app.editor.active().unwrap().is_dirty());

    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

fn dirty_app() -> (App, DocumentId) {
    let mut app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    app.config.hot_exit = Some(false);
    app.editor.new_untitled();
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("draft".into()), 1, 4);
    let id = doc.id;
    (app, id)
}

#[test]
fn close_requests_do_not_interrupt_a_pending_save_as() {
    let (mut app, id) = dirty_app();
    app.close_tab(0);
    app.answer_save_prompt(SaveChoice::Save);

    for request in [
        CloseRequest::CloseProject,
        CloseRequest::Quit,
        CloseRequest::Tabs(vec![id]),
    ] {
        app.request_close(request);
        let prompt = app.closing.continuation.as_ref().unwrap();
        assert_eq!(prompt.request, CloseRequest::Tabs(vec![id]));
        assert_eq!(prompt.dirty, [id]);
        assert!(app.closing.prompt.is_none());
        assert!(app.editor.document(id).unwrap().is_dirty());
        assert!(!app.project_closed && !app.closing.quit_requested);
    }

    app.request_quit();
    app.request_close_project();
    assert!(app.closing.prompt.is_none());
    assert_eq!(
        app.closing.continuation.as_ref().unwrap().request,
        CloseRequest::Tabs(vec![id])
    );
    app.on_picked(Pick::SaveAs(id), None);
    assert!(app.closing.continuation.is_none());
    assert!(app.editor.document(id).unwrap().is_dirty());
    assert!(!app.closing_pending());
}

#[test]
fn closing_a_project_finishes_the_original_save_as_before_removal() {
    let (saving, id) = dirty_app();
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(dirty_app().0);
    winc.app(first).request_close_project();
    winc.app(first).answer_save_prompt(SaveChoice::Save);
    winc.app(first).request_quit();
    assert!(!winc.settle_for_test().1);
    assert_eq!(winc.keys(), [first, second]);
    assert_eq!(
        winc.app(first)
            .closing
            .continuation
            .as_ref()
            .unwrap()
            .request,
        CloseRequest::CloseProject
    );

    let path =
        std::env::temp_dir().join(format!("winc-save-before-close-{}.txt", std::process::id()));
    winc.app(first)
        .on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
    assert!(!winc.settle_for_test().1);
    assert_eq!(winc.keys(), [second]);
    assert!(winc.app(second).editor.active().unwrap().is_dirty());
    assert!(
        winc.app(second).closing.prompt.is_some(),
        "关闭项目后的退出请求应交给剩余项目"
    );
    assert_eq!(
        winc.app(second).closing.prompt.as_ref().unwrap().request,
        CloseRequest::Quit
    );
    winc.app(second).answer_save_prompt(SaveChoice::Cancel);
    assert!(!winc.settle_for_test().1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn hot_exit_cannot_close_a_project_while_save_as_is_pending() {
    let root = std::env::temp_dir().join(format!("winc-pending-hot-exit-{}", std::process::id()));
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let (mut app, id) = dirty_app();
    app.editor.open_folder(&folder).unwrap();
    app.config.hot_exit = Some(true);
    app.backup.root = Some(root.join("backup"));
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(app);
    let second = winc.add_detached(dirty_app().0);
    winc.app(first).close_tab(0);
    winc.app(first).answer_save_prompt(SaveChoice::Save);
    winc.app(first).request_close_project();
    assert!(!winc.settle_for_test().1);
    assert_eq!(winc.keys(), [first, second]);
    assert_eq!(
        winc.app(first)
            .closing
            .continuation
            .as_ref()
            .unwrap()
            .request,
        CloseRequest::Tabs(vec![id])
    );
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    winc.app(first).on_picked(Pick::SaveAs(id), None);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn quitting_waits_for_another_projects_save_as() {
    let (mut saving, id) = dirty_app();
    saving.close_tab(0);
    saving.answer_save_prompt(SaveChoice::Save);
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.set_active(second);
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).closing.prompt.is_none());
    assert_eq!(
        winc.app(first)
            .closing
            .continuation
            .as_ref()
            .unwrap()
            .request,
        CloseRequest::Tabs(vec![id])
    );

    let path =
        std::env::temp_dir().join(format!("winc-save-before-quit-{}.txt", std::process::id()));
    winc.app(first)
        .on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
    assert!(winc.app(first).editor.document(id).is_none());
    assert!(winc.settle_for_test().1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cancelling_save_as_keeps_the_document_and_quit_can_be_cancelled() {
    let (mut saving, id) = dirty_app();
    saving.close_tab(0);
    saving.answer_save_prompt(SaveChoice::Save);
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).closing.prompt.is_none());

    winc.app(first).on_picked(Pick::SaveAs(id), None);
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    assert!(!winc.app(first).closing_pending());
    assert!(!winc.app(second).closing.quit_requested);
    assert_eq!(winc.keys(), [first, second]);
}

#[test]
fn ordinary_save_as_prevents_project_close_until_the_picker_returns() {
    let (mut app, id) = dirty_app();
    app.pick(Pick::SaveAs(id));
    app.request_close_project();
    assert!(app.closing.prompt.is_none());
    assert!(app.editor.document(id).unwrap().is_dirty());
    assert!(!app.project_closed);
    assert!(app.closing_pending());

    app.on_picked(Pick::SaveAs(id), None);
    assert!(!app.closing_pending());
    assert!(app.editor.document(id).unwrap().is_dirty());
    app.request_close_project();
    assert_eq!(
        app.closing.prompt.as_ref().unwrap().request,
        CloseRequest::CloseProject
    );
}

#[test]
fn quitting_waits_for_an_ordinary_save_as_in_another_project() {
    let (mut saving, id) = dirty_app();
    saving.pick(Pick::SaveAs(id));
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).closing.prompt.is_none());
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());

    let path = std::env::temp_dir().join(format!(
        "winc-ordinary-save-before-quit-{}.txt",
        std::process::id()
    ));
    winc.app(first)
        .on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
    assert!(winc.settle_for_test().1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cancelling_an_ordinary_save_as_during_quit_keeps_the_document() {
    let (mut saving, id) = dirty_app();
    saving.pick(Pick::SaveAs(id));
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    winc.app(first).on_picked(Pick::SaveAs(id), None);
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    assert!(!winc.app(first).closing_pending());
    assert!(!winc.app(second).closing.quit_requested);
}

#[test]
fn sequential_save_as_cancellation_keeps_the_remaining_project_open() {
    let (mut app, first) = dirty_app();
    app.editor.new_untitled();
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("second".into()), 1, 4);
    let second = doc.id;
    app.request_close_project();
    app.answer_save_prompt(SaveChoice::Save);
    let path =
        std::env::temp_dir().join(format!("winc-sequential-save-{}.txt", std::process::id()));
    app.on_picked(Pick::SaveAs(first), Some(path.clone()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
    assert_eq!(app.save_as_picker, Some(second));
    assert_eq!(
        app.closing.continuation.as_ref().unwrap().request,
        CloseRequest::CloseProject
    );
    app.request_quit();
    app.on_picked(Pick::SaveAs(second), None);
    assert!(!app.closing_pending() && !app.project_closed && !app.closing.quit_requested);
    assert!(!app.editor.document(first).unwrap().is_dirty());
    assert!(app.editor.document(second).unwrap().is_dirty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cancelling_or_failing_save_as_for_quit_cancels_the_whole_quit() {
    let parent = std::env::temp_dir().join(format!("winc-save-parent-{}", std::process::id()));
    std::fs::write(&parent, "occupied").unwrap();
    let invalid = parent.join("draft.txt");
    for path in [None, Some(invalid)] {
        let (mut saving, id) = dirty_app();
        saving.request_quit();
        saving.answer_save_prompt(SaveChoice::Save);
        let mut winc = crate::winc::Winc::new(None, None);
        let first = winc.add_detached(saving);
        let second = winc.add_detached(App::new(
            Editor::default(),
            None,
            crate::bench::Bench::off(),
            Config::default(),
        ));
        winc.app(second).request_quit();
        assert!(!winc.settle_for_test().1);
        assert!(winc.app(first).quit_pending());
        assert!(winc.app(first).closing.prompt.is_none());

        winc.app(first).on_picked(Pick::SaveAs(id), path);
        assert!(!winc.settle_for_test().1);
        assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
        assert!(!winc.app(first).closing_pending());
        assert!(
            !winc.app(first).closing.quit_requested && !winc.app(second).closing.quit_requested
        );
        assert_eq!(winc.keys(), [first, second]);
    }
    std::fs::remove_file(parent).unwrap();
}

#[test]
fn save_as_started_after_project_agrees_to_quit_blocks_exit() {
    let mut first_app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    first_app.editor.new_untitled();
    let id = first_app.editor.active().unwrap().id;
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(first_app);
    let second = winc.add_detached(dirty_app().0);
    winc.app(first).request_quit();
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(second).closing.prompt.is_some());
    winc.app(first).pick(Pick::SaveAs(id));
    assert_eq!(winc.app(first).save_as_picker, Some(id));
    winc.app(second).answer_save_prompt(SaveChoice::Discard);
    assert!(!winc.settle_for_test().1);

    let path = std::env::temp_dir().join(format!("winc-late-save-{}.txt", std::process::id()));
    winc.app(first)
        .on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    assert!(winc.settle_for_test().1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cancelling_a_late_save_as_cancels_quit_and_keeps_dirty_documents() {
    let mut first_app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    first_app.config.hot_exit = Some(false);
    first_app.editor.new_untitled();
    let id = first_app.editor.active().unwrap().id;
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(first_app);
    let second = winc.add_detached(dirty_app().0);
    winc.app(first).request_quit();
    assert!(!winc.settle_for_test().1);
    winc.app(first).editor.document_mut(id).unwrap().execute(
        Command::Insert("new draft".into()),
        1,
        4,
    );
    winc.app(first).pick(Pick::SaveAs(id));
    winc.app(second).answer_save_prompt(SaveChoice::Discard);
    assert!(!winc.settle_for_test().1);
    winc.app(first).on_picked(Pick::SaveAs(id), None);
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(first).closing.prompt.is_none());
    assert!(!winc.app(first).closing.quit_requested && !winc.app(second).closing.quit_requested);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
}

#[test]
fn waited_tab_stays_open_while_another_document_is_being_saved_as() {
    let mut app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    app.editor.new_untitled();
    let waited = app.editor.active().unwrap().id;
    app.start_one_off(&crate::cli::Args {
        wait: true,
        ..Default::default()
    });
    app.editor.new_untitled();
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("draft".into()), 1, 4);
    let saving = doc.id;
    app.pick(Pick::SaveAs(saving));
    app.close_tab(0);
    app.finish_waiting();
    assert!(app.editor.document(waited).is_some());
    assert!(!app.closing.quit_requested);

    app.on_picked(Pick::SaveAs(saving), None);
    assert!(app.editor.document(waited).is_some());
    assert!(!app.closing.quit_requested);
    app.close_tab(0);
    app.finish_waiting();
    assert!(app.editor.document(waited).is_none());
    assert!(app.editor.document(saving).unwrap().is_dirty());
    assert_eq!(
        app.closing.prompt.as_ref().unwrap().request,
        CloseRequest::Quit
    );
    app.answer_save_prompt(SaveChoice::Cancel);
    assert!(!app.closing.quit_requested);
}

#[test]
fn another_save_picker_cannot_consume_the_pending_continuation() {
    let (mut app, id) = dirty_app();
    app.close_tab(0);
    app.answer_save_prompt(SaveChoice::Save);
    app.editor.new_untitled();
    let other = app.editor.active().unwrap().id;
    app.pick(Pick::SaveAs(other));
    app.on_picked(Pick::SaveAs(other), None);
    assert_eq!(app.save_as_picker, Some(id));
    assert_eq!(
        app.closing.continuation.as_ref().unwrap().request,
        CloseRequest::Tabs(vec![id])
    );
    app.on_picked(Pick::SaveAs(id), None);
    assert!(app.editor.document(id).unwrap().is_dirty());
    assert!(!app.closing_pending());
}

#[test]
fn direct_close_and_quit_wait_for_ordinary_save_as() {
    let root = std::env::temp_dir().join(format!("winc-direct-save-as-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("blocked"), "occupied").unwrap();
    for quit in [false, true] {
        for outcome in ["success", "cancel", "failure"] {
            let (app, id) = dirty_app();
            let mut winc = crate::winc::Winc::new(None, None);
            let first = winc.add_detached(app);
            let second = winc.add_detached(dirty_app().0);
            winc.app(first).pick(Pick::SaveAs(id));
            for _ in 0..3 {
                if quit {
                    winc.app(first).request_quit();
                } else {
                    winc.app(first).request_close_project();
                }
            }
            assert!(!winc.settle_for_test().1);
            assert!(winc.app(first).closing.prompt.is_none());
            assert_eq!(winc.app(first).quit_pending(), quit);
            let target = root.join(format!("{quit}-{outcome}.txt"));
            let path = match outcome {
                "success" => Some(target.clone()),
                "failure" => Some(root.join("blocked").join("draft.txt")),
                _ => None,
            };
            winc.app(first).on_picked(Pick::SaveAs(id), path);
            if outcome == "success" {
                assert_eq!(std::fs::read_to_string(&target).unwrap(), "draft");
                assert!(!winc.settle_for_test().1);
                if quit {
                    assert!(winc.app(first).closing.quit_requested);
                    assert_eq!(
                        winc.app(second).closing.prompt.as_ref().unwrap().request,
                        CloseRequest::Quit
                    );
                    winc.app(second).answer_save_prompt(SaveChoice::Cancel);
                    assert!(!winc.settle_for_test().1);
                    assert!(winc.app(first).editor.document(id).is_some());
                } else {
                    assert_eq!(winc.keys(), [second]);
                }
            } else {
                assert!(!winc.settle_for_test().1);
                assert_eq!(winc.keys(), [first, second]);
                assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
                assert!(!winc.app(first).closing_pending());
                assert!(!winc.app(first).closing.quit_requested);
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn direct_close_and_quit_wait_for_save_as_from_tab_close() {
    let root = std::env::temp_dir().join(format!("winc-tab-save-as-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("blocked"), "occupied").unwrap();
    for quit in [false, true] {
        for outcome in ["success", "cancel", "failure"] {
            let (mut app, id) = dirty_app();
            app.close_tab(0);
            app.answer_save_prompt(SaveChoice::Save);
            let mut winc = crate::winc::Winc::new(None, None);
            let first = winc.add_detached(app);
            let second = winc.add_detached(dirty_app().0);
            if quit {
                winc.app(first).request_quit();
            } else {
                winc.app(first).request_close_project();
            }
            assert_eq!(
                winc.app(first)
                    .closing
                    .continuation
                    .as_ref()
                    .unwrap()
                    .request,
                CloseRequest::Tabs(vec![id])
            );
            assert!(!winc.settle_for_test().1);
            let target = root.join(format!("{quit}-{outcome}.txt"));
            let path = match outcome {
                "success" => Some(target.clone()),
                "failure" => Some(root.join("blocked").join("draft.txt")),
                _ => None,
            };
            winc.app(first).on_picked(Pick::SaveAs(id), path);
            assert!(!winc.settle_for_test().1);
            if outcome == "success" {
                assert_eq!(std::fs::read_to_string(&target).unwrap(), "draft");
                if quit {
                    assert!(winc.app(first).editor.document(id).is_none());
                    assert!(winc.app(first).closing.quit_requested);
                    assert_eq!(
                        winc.app(second).closing.prompt.as_ref().unwrap().request,
                        CloseRequest::Quit
                    );
                } else {
                    assert_eq!(winc.keys(), [second]);
                }
            } else {
                assert_eq!(winc.keys(), [first, second]);
                assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
                assert!(!winc.app(first).closing_pending());
                assert!(!winc.app(first).closing.quit_requested);
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelling_global_quit_clears_quit_deferred_by_another_project() {
    let (mut saving, id) = dirty_app();
    saving.pick(Pick::SaveAs(id));
    saving.request_quit();
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    let third = winc.add_detached(dirty_app().0);
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    assert_eq!(
        winc.app(third).closing.prompt.as_ref().unwrap().request,
        CloseRequest::Quit
    );
    winc.app(third).answer_save_prompt(SaveChoice::Cancel);
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(first).quit_pending());

    let path = std::env::temp_dir().join(format!("winc-cancel-global-{}.txt", std::process::id()));
    winc.app(first)
        .on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(first).closing.quit_requested);
    assert!(winc.app(third).closing.prompt.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cancelling_or_failing_project_save_as_cancels_deferred_quit() {
    let root = std::env::temp_dir().join(format!("winc-project-save-as-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("blocked"), "occupied").unwrap();
    for path in [None, Some(root.join("blocked").join("draft.txt"))] {
        let (mut app, id) = dirty_app();
        app.request_close_project();
        app.answer_save_prompt(SaveChoice::Save);
        app.request_quit();
        let mut winc = crate::winc::Winc::new(None, None);
        let first = winc.add_detached(app);
        let second = winc.add_detached(dirty_app().0);
        assert!(!winc.settle_for_test().1);
        assert!(winc.app(first).quit_pending());
        winc.app(first).on_picked(Pick::SaveAs(id), path);
        assert!(!winc.settle_for_test().1);
        assert_eq!(winc.keys(), [first, second]);
        assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
        assert!(!winc.app(first).closing_pending());
        assert!(!winc.app(first).closing.quit_requested);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deferred_tab_close_preserves_wait_until_that_tab_closes() {
    let mut app = App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    );
    app.editor.new_untitled();
    let waited = app.editor.active().unwrap().id;
    app.start_one_off(&crate::cli::Args {
        wait: true,
        ..Default::default()
    });
    app.editor.new_untitled();
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("draft".into()), 1, 4);
    let saving = doc.id;
    app.pick(Pick::SaveAs(saving));
    app.close_tab(0);
    app.finish_waiting();
    assert!(app.editor.document(waited).is_some());
    assert!(!app.closing.quit_requested);

    let path = std::env::temp_dir().join(format!("winc-wait-deferred-{}.txt", std::process::id()));
    app.on_picked(Pick::SaveAs(saving), Some(path.clone()));
    assert!(app.editor.document(waited).is_none());
    assert!(app.editor.document(saving).is_some());
    app.finish_waiting();
    assert!(app.closing.quit_requested);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn deferred_quit_waits_for_each_save_in_a_close_sequence() {
    let (mut app, first) = dirty_app();
    app.editor.new_untitled();
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("second".into()), 1, 4);
    let second = doc.id;
    app.close_tabs(vec![first, second]);
    app.answer_save_prompt(SaveChoice::Save);
    app.request_quit();
    let root = std::env::temp_dir().join(format!("winc-deferred-sequence-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    app.on_picked(Pick::SaveAs(first), Some(root.join("first.txt")));
    assert_eq!(app.save_as_picker, Some(second));
    assert!(app.quit_pending());
    assert!(!app.closing.quit_requested);
    app.on_picked(Pick::SaveAs(second), Some(root.join("second.txt")));
    assert!(app.editor.documents.is_empty());
    assert!(app.closing.quit_requested);
    assert_eq!(
        std::fs::read_to_string(root.join("first.txt")).unwrap(),
        "draft"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("second.txt")).unwrap(),
        "second"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deferred_project_close_rechecks_terminal_confirmation() {
    let (mut app, id) = dirty_app();
    app.pick(Pick::SaveAs(id));
    app.request_close_project();
    let mut terminal = crate::app::terminal::TermTab::new(1, Default::default());
    terminal.process = "vim".into();
    app.terminal.tabs.push(terminal);
    app.closing.confirming = Some(CloseRequest::CloseProject);

    let path =
        std::env::temp_dir().join(format!("winc-deferred-terminal-{}.txt", std::process::id()));
    app.on_picked(Pick::SaveAs(id), Some(path.clone()));
    assert!(app.editor.document(id).is_some());
    assert_eq!(app.closing.confirming, Some(CloseRequest::CloseProject));
    app.handle_event(UserEvent::TerminalQuit(false));
    assert!(app.editor.document(id).is_some());
    assert!(!app.project_closed);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn deferred_close_preserves_the_other_tab_during_save_as() {
    let (mut app, saving) = dirty_app();
    app.editor.new_untitled();
    let other = app.editor.active_mut().unwrap();
    other.execute(Command::Insert("other".into()), 1, 4);
    let other = other.id;
    app.pick(Pick::SaveAs(saving));
    app.close_tab(1);
    let path = std::env::temp_dir().join(format!("winc-deferred-tab-{}.txt", std::process::id()));
    app.on_picked(Pick::SaveAs(saving), Some(path.clone()));
    assert_eq!(
        app.closing.prompt.as_ref().unwrap().request,
        CloseRequest::Tabs(vec![other])
    );
    app.answer_save_prompt(SaveChoice::Discard);
    assert!(app.editor.document(other).is_none());
    assert!(app.editor.document(saving).is_some());
    assert!(!app.closing.quit_requested);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn close_folder_and_one_off_switch_wait_for_save_as() {
    let root = std::env::temp_dir().join(format!("winc-save-folder-{}", std::process::id()));
    let original = root.join("original");
    let target = root.join("target");
    std::fs::create_dir_all(&original).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(root.join("blocked"), "occupied").unwrap();
    let original = winc_config::files::canonicalize(original).unwrap();
    let target = winc_config::files::canonicalize(target).unwrap();
    for switch in [false, true] {
        for outcome in ["success", "cancel", "failure"] {
            let (mut app, id) = dirty_app();
            app.editor.open_folder(&original).unwrap();
            let mut winc = crate::winc::Winc::new(None, None);
            let key = winc.add_detached(app);
            winc.app(key).pick(Pick::SaveAs(id));
            if switch {
                assert!(winc.open_folder_for_test(&target));
            } else {
                winc.app(key).close_folder();
            }
            assert_eq!(winc.app(key).editor.root(), Some(original.as_path()));
            assert!(winc.app(key).closing_pending());
            let saved = root.join(format!("{switch}-{outcome}.txt"));
            let result = match outcome {
                "success" => Some(saved.clone()),
                "failure" => Some(root.join("blocked").join("draft.txt")),
                _ => None,
            };
            winc.app(key).on_picked(Pick::SaveAs(id), result);
            if outcome == "success" {
                assert_eq!(std::fs::read_to_string(saved).unwrap(), "draft");
                assert_eq!(
                    winc.app(key).editor.root(),
                    switch.then_some(target.as_path())
                );
                assert!(winc.app(key).editor.document(id).is_none());
            } else {
                assert_eq!(winc.app(key).editor.root(), Some(original.as_path()));
                assert!(winc.app(key).editor.document(id).unwrap().is_dirty());
                assert!(!winc.app(key).closing_pending());
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

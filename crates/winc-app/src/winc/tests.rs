//! `winc` 的测试辅助：测试中没有事件循环，直接加入窗口、执行处理完一个事件后的那一步（见
//! `app/snapshot/windows.rs`）。

use super::*;

impl Winc {
    /// 加入一个窗口，返回分到的编号（测试没有事件循环，用不到代理）。
    pub(crate) fn add_detached(&mut self, app: App) -> WindowKey {
        let (key, _) = self.reserve();
        self.add(key, app);
        key
    }

    /// 编号为 `key` 的窗口。
    ///
    /// # Panics
    ///
    /// 没有这个窗口时 panic。
    pub(crate) fn app(&mut self, key: WindowKey) -> &mut App {
        self.by_key(key).expect("没有这个窗口")
    }

    /// 测试用：按加入顺序返回所有窗口的编号。
    pub(crate) fn keys(&self) -> Vec<WindowKey> {
        self.windows.iter().map(|slot| slot.key).collect()
    }

    /// 测试用：把当前窗口设为 `key`，不经过窗口交接（测试里没有系统窗口）。
    pub(crate) fn set_active(&mut self, key: WindowKey) {
        self.active = Some(key);
    }

    /// 测试用：当前项目。
    pub(crate) fn active_for_test(&self) -> Option<WindowKey> {
        self.active_key()
    }

    /// 测试用：把窗口交给项目 `key`，与点击侧栏条目走同一条路。
    pub(crate) fn activate_for_test(&mut self, key: WindowKey) {
        self.activate(key);
    }

    /// 测试用：跑一遍处理完事件后的那一步，返回窗口的请求和是否该结束事件循环。
    pub(crate) fn settle_for_test(&mut self) -> (Vec<Request>, bool) {
        self.settle()
    }

    /// 测试用：返回打开 `targets` 的窗口，语义见 `window_for`。
    pub(crate) fn window_for_test(
        &mut self,
        targets: &[Target],
    ) -> Result<WindowKey, Option<PathBuf>> {
        self.window_for(targets)
    }

    /// 测试用：处理项目发出的打开目录请求。
    pub(crate) fn open_folder_for_test(&mut self, folder: &Path) -> bool {
        let folder =
            winc_config::files::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
        self.try_open_folder(&folder)
    }

    /// 测试用：记下其他窗口开着的目录。
    pub(crate) fn remember_windows_for_test(&mut self) {
        self.remember_windows();
    }
}

fn test_app() -> App {
    App::new(
        Editor::default(),
        None,
        Bench::off(),
        winc_config::Config::default(),
    )
}

fn dirty_app() -> App {
    let mut app = test_app();
    app.config.hot_exit = Some(false);
    app.editor.new_untitled();
    app.editor
        .active_mut()
        .unwrap()
        .execute(winc_editor::Command::Insert("draft".into()), 1, 4);
    app
}

fn test_folder(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("winc-project-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    winc_config::files::canonicalize(&path).unwrap()
}

fn normal_winc() -> Winc {
    Winc::new(
        None,
        Some(Template {
            config_path: None,
            state_path: None,
            backup_root: None,
        }),
    )
}

#[test]
fn stale_project_keys_leave_the_active_project_untouched() {
    let mut winc = normal_winc();
    let active = winc.add_detached(dirty_app());
    let closed = winc.add_detached(test_app());
    winc.close_project(closed);
    winc.settle();

    assert!(!winc.activate(closed));
    winc.close_project(closed);
    assert_eq!(winc.active, Some(active));
    assert!(winc.by_key(closed).is_none());
    assert_eq!(winc.keys(), [active]);
    assert!(winc.app(active).editor.active().unwrap().is_dirty());
    assert!(!winc.app(active).project_closed);
}

#[test]
fn quit_activates_each_project_and_keeps_the_original_for_restore() {
    let mut winc = normal_winc();
    let first = winc.add_detached(test_app());
    let second = winc.add_detached(dirty_app());
    let third = winc.add_detached(dirty_app());
    winc.app(first).request_quit();

    assert!(!winc.settle().1);
    assert_eq!(winc.active_key(), Some(third));
    assert!(winc.app(third).save_prompt.is_some());
    assert!(winc.app(second).save_prompt.is_none());

    // 取消时，正在询问的项目保持活动，整个退出流程停止。
    winc.app(third).save_prompt = None;
    assert!(!winc.settle().1);
    assert_eq!(winc.active_key(), Some(third));
    assert!(!winc.app(first).quit_requested);
    assert!(winc.quitting.is_none());

    winc.activate(first);
    winc.app(first).request_quit();
    assert!(!winc.settle().1);
    for key in [third, second] {
        assert_eq!(winc.active_key(), Some(key));
        assert!(winc.app(key).save_prompt.take().is_some());
        winc.app(key)
            .editor
            .active_mut()
            .unwrap()
            .history
            .mark_saved();
        winc.app(key).request_quit();
        let (_, exit) = winc.settle();
        assert_eq!(exit, key == second);
    }
    assert_eq!(winc.active_key(), Some(first));
}

#[test]
fn closed_quit_origin_still_asks_remaining_projects() {
    use crate::app::CloseRequest;

    let mut winc = normal_winc();
    let first = winc.add_detached(test_app());
    let second = winc.add_detached(dirty_app());
    winc.app(first).request_close_project();
    winc.app(first).request_quit();
    assert!(winc.app(first).project_closed && winc.app(first).quit_requested);

    assert!(!winc.settle().1);
    assert_eq!(winc.keys(), [second]);
    assert_eq!(
        winc.app(second).save_prompt.as_ref().unwrap().request,
        CloseRequest::Quit
    );
    winc.app(second).save_prompt = None;
    assert!(!winc.settle().1);
    assert!(winc.quitting.is_none());
    assert!(winc.app(second).editor.active().unwrap().is_dirty());
}

#[test]
fn cancelling_quit_keeps_independent_save_continuations() {
    use crate::app::CloseRequest;

    for formatted in [false, true] {
        let mut winc = normal_winc();
        let closing = winc.add_detached(dirty_app());
        let id = winc.app(closing).editor.active().unwrap().id;
        winc.app(closing).request_close_project();
        let mut prompt = winc.app(closing).save_prompt.take().unwrap();
        prompt.deferred = Some(CloseRequest::Quit);
        winc.app(closing).save_continuation = Some(prompt);
        winc.app(closing).save_as_picker = Some(id);

        let saving = winc.add_detached(dirty_app());
        let saving_id = winc.app(saving).editor.active().unwrap().id;
        winc.app(saving).request_quit();
        let prompt = winc.app(saving).save_prompt.take().unwrap();
        if formatted {
            winc.app(saving).format.then_close = Some(prompt);
        } else {
            winc.app(saving).save_continuation = Some(prompt);
            winc.app(saving).save_as_picker = Some(saving_id);
        }
        let starter = winc.add_detached(test_app());
        let canceller = winc.add_detached(dirty_app());
        winc.app(starter).request_quit();
        assert!(!winc.settle().1);
        assert_eq!(winc.active_key(), Some(canceller));
        winc.app(canceller).save_prompt = None;
        assert!(!winc.settle().1);
        assert!(winc.quitting.is_none());

        let app = winc.app(closing);
        let prompt = app.save_continuation.as_ref().unwrap();
        assert_eq!(prompt.request, CloseRequest::CloseProject);
        assert_eq!(prompt.dirty, [id]);
        assert!(prompt.deferred.is_none());
        assert_eq!(app.save_as_picker, Some(id));
        assert!(app.editor.document(id).unwrap().is_dirty());
        assert!(!app.quit_pending());

        let app = winc.app(saving);
        let prompt = if formatted {
            app.format.then_close.as_ref().unwrap()
        } else {
            assert_eq!(app.save_as_picker, Some(saving_id));
            app.save_continuation.as_ref().unwrap()
        };
        assert_eq!(prompt.request, CloseRequest::Tabs(Vec::new()));
        assert_eq!(prompt.dirty, [saving_id]);
        assert!(app.editor.document(saving_id).unwrap().is_dirty());
        assert!(!app.quit_pending());
        assert!(!winc.settle().1);
    }
}

#[test]
fn cancelling_quit_neutralizes_other_pending_quit_dialogs() {
    use crate::app::CloseRequest;

    let mut winc = normal_winc();
    let prompt = winc.add_detached(dirty_app());
    let id = winc.app(prompt).editor.active().unwrap().id;
    winc.app(prompt).request_quit();
    let terminal = winc.add_detached(test_app());
    winc.app(terminal).quit_confirming = Some(CloseRequest::Quit);
    let starter = winc.add_detached(test_app());
    let canceller = winc.add_detached(dirty_app());
    winc.app(starter).request_quit();
    assert!(!winc.settle().1);
    assert_eq!(winc.active_key(), Some(canceller));
    winc.app(canceller).save_prompt = None;
    assert!(!winc.settle().1);

    let app = winc.app(prompt);
    let pending = app.save_prompt.as_ref().unwrap();
    assert_eq!(pending.request, CloseRequest::Tabs(Vec::new()));
    assert_eq!(pending.dirty, [id]);
    assert!(app.editor.document(id).unwrap().is_dirty());
    assert!(!app.quit_pending());
    assert_eq!(
        winc.app(terminal).quit_confirming,
        Some(CloseRequest::Tabs(Vec::new()))
    );
    assert!(!winc.app(terminal).quit_pending());
    assert!(!winc.settle().1);
}

#[test]
fn stale_quit_targets_are_skipped() {
    let mut winc = normal_winc();
    let active = winc.add_detached(test_app());
    winc.app(active).request_quit();
    winc.quitting = Some(Quitting {
        asking: None,
        rest: vec![WindowKey(u32::MAX)],
        active: Some(active),
    });
    assert!(winc.settle().1);
    assert_eq!(winc.active_key(), Some(active));
    assert!(winc.app(active).quit_requested);
}

#[test]
fn folder_routing_reuses_only_the_current_empty_start_page_and_deduplicates() {
    let first = test_folder("routing-first");
    let second = test_folder("routing-second");
    let mut winc = normal_winc();
    let a = winc.add_detached(test_app());
    let b = winc.add_detached(test_app());

    assert!(winc.try_open_folder(&first));
    assert_eq!(winc.app(a).editor.root(), Some(first.as_path()));
    assert_eq!(winc.keys(), [a, b]);

    winc.activate(b);
    let alias = first.join(".");
    assert!(winc.open_folder_for_test(&alias));
    assert_eq!(winc.active_key(), Some(a));
    assert!(winc.app(b).editor.root().is_none());

    assert!(!winc.try_open_folder(&second));
    assert_eq!(winc.app(a).editor.root(), Some(first.as_path()));
    winc.activate(b);
    assert!(winc.try_open_folder(&second));
    assert_eq!(winc.app(b).editor.root(), Some(second.as_path()));
    drop(winc);
    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn a_start_page_with_a_document_or_terminal_is_not_reused() {
    let folder = test_folder("occupied");
    let mut winc = normal_winc();
    let key = winc.add_detached(dirty_app());
    assert!(!winc.try_open_folder(&folder));
    assert!(winc.app(key).editor.active().unwrap().is_dirty());

    winc.app(key).editor.close(0);
    winc.app(key).terminal.want.push(Vec::new());
    assert!(!winc.try_open_folder(&folder));
    assert!(winc.app(key).editor.root().is_none());
    assert_eq!(winc.app(key).terminal.want.len(), 1);
    drop(winc);
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn restore_windows_preserves_a_start_page_with_documents_or_terminal() {
    let folder = test_folder("restore-occupied");
    for terminal_only in [false, true] {
        let mut winc = normal_winc();
        let key = winc.add_detached(if terminal_only {
            test_app()
        } else {
            dirty_app()
        });
        if terminal_only {
            winc.app(key).terminal.want.push(Vec::new());
        }

        winc.restore_windows(std::slice::from_ref(&folder));
        let (requests, _) = winc.settle();
        assert_eq!(
            requests,
            [Request::OpenFolder(folder.clone()), Request::Focus(key)]
        );
        assert!(winc.app(key).editor.root().is_none());
        if terminal_only {
            assert_eq!(winc.app(key).terminal.want.len(), 1);
        } else {
            assert!(winc.app(key).editor.active().unwrap().is_dirty());
        }
    }
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn two_blank_projects_with_drafts_do_not_share_a_backup_lock() {
    let root = test_folder("blank-backup-lock");
    let mut first_app = dirty_app();
    first_app.config.hot_exit = Some(true);
    first_app.backup.root = Some(root.clone());
    let mut second_app = dirty_app();
    second_app.config.hot_exit = Some(true);
    second_app.backup.root = Some(root.clone());
    let mut winc = normal_winc();
    let first = winc.add_detached(first_app);
    let second = winc.add_detached(second_app);

    winc.app(first).request_quit();
    assert!(!winc.settle().1);
    assert!(winc.app(first).quit_requested);
    assert!(winc.app(second).save_prompt.is_some());
    winc.app(second).save_prompt = None;
    assert!(!winc.settle().1);
    assert!(!winc.app(first).quit_requested);
    assert!(winc.app(first).editor.active().unwrap().is_dirty());
    assert!(winc.app(second).editor.active().unwrap().is_dirty());
    drop(winc);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn quitting_and_restarting_recovers_blank_drafts_and_directory_projects() {
    for blank_active in [false, true] {
        for restore in [false, true] {
            let root = test_folder(&format!("recovery-{blank_active}-{restore}"));
            let folder = root.join("project");
            std::fs::create_dir_all(&folder).unwrap();
            let folder = winc_config::files::canonicalize(&folder).unwrap();
            let state_path = root.join("state.json");
            let backup_root = root.join("backups");
            let template = Template {
                config_path: None,
                state_path: Some(state_path.clone()),
                backup_root: Some(backup_root.clone()),
            };
            let mut blank = dirty_app();
            blank.config.hot_exit = Some(true);
            blank.config.restore_folder = Some(restore);
            blank.state_path = Some(state_path.clone());
            blank.backup.root = Some(backup_root.clone());
            let mut project = dirty_app();
            project.editor.open_folder(&folder).unwrap();
            project.editor.active_mut().unwrap().execute(
                winc_editor::Command::Insert(" project".into()),
                1,
                4,
            );
            project.config = blank.config.clone();
            project.state_path = Some(state_path.clone());
            project.backup.root = Some(backup_root.clone());
            let mut winc = Winc::new(None, Some(template.clone()));
            let blank_key = winc.add_detached(blank);
            let project_key = winc.add_detached(project);
            let active = if blank_active { blank_key } else { project_key };
            winc.activate(active);
            winc.app(active).request_quit();
            assert!(winc.settle().1);
            assert_eq!(winc.active_key(), Some(active));
            winc.windows.sort_by_key(|slot| slot.key == active);
            for slot in &mut winc.windows {
                slot.app.prepare_exit();
            }
            winc.remember_windows();
            drop(winc);

            let state = State::load_from(&state_path);
            assert_eq!(
                state.last.as_deref(),
                (restore && !blank_active).then_some(folder.as_path())
            );
            assert_eq!(
                state.others,
                if restore && blank_active {
                    vec![folder.clone()]
                } else {
                    vec![]
                }
            );
            let mut editor = Editor::default();
            let session = state
                .last_session()
                .filter(|session| crate::open_session_folder(&mut editor, session));
            let restored = crate::app::restore_at_launch(&mut editor, session, Some(&backup_root));
            let mut initial = test_app();
            initial.editor = editor;
            initial.state = state.clone();
            initial.state_path = Some(state_path.clone());
            initial.backup.root = Some(backup_root.clone());
            initial.adopt_restored(restored);
            let mut next = Winc::new(None, Some(template));
            let initial_key = next.add_detached(initial);
            next.restore_empty_project();
            next.restore_windows(&state.others);
            let (requests, _) = next.settle();
            for request in requests {
                match request {
                    Request::OpenFolder(path) => {
                        if !next.try_open_folder(&path) {
                            let key = next.add_project(Editor::default()).unwrap();
                            next.activate(key);
                            next.app(key).open_folder_now(&path);
                        }
                    }
                    Request::Focus(key) => {
                        next.activate(key);
                    }
                    other => panic!("unexpected request: {other:?}"),
                }
            }
            assert_eq!(next.active_key(), Some(initial_key));
            let blank = next
                .windows
                .iter()
                .find(|slot| slot.app.editor.root().is_none())
                .unwrap();
            let text = |app: &App| {
                app.editor
                    .active()
                    .unwrap()
                    .text()
                    .rope()
                    .unwrap()
                    .to_string()
            };
            assert_eq!(text(&blank.app), "draft");
            assert!(blank.app.editor.active().unwrap().is_dirty());
            assert_eq!(
                next.app(initial_key).editor.root().is_some(),
                restore && !blank_active
            );
            if restore {
                let project = next
                    .windows
                    .iter()
                    .find(|slot| slot.app.editor.root() == Some(folder.as_path()))
                    .unwrap();
                assert_eq!(text(&project.app), "draft project");
                assert!(project.app.editor.active().unwrap().is_dirty());
            } else {
                assert!(
                    next.windows
                        .iter()
                        .all(|slot| slot.app.editor.root().is_none())
                );
            }
            drop(next);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn restoring_blank_drafts_keeps_the_original_directory_tabs_for_another_launch() {
    let root = test_folder("recover-active-selection");
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let selected = folder.join("selected.txt");
    std::fs::write(&selected, "saved").unwrap();
    let backup_root = root.join("backups");
    let mut blank = dirty_app();
    blank.config.hot_exit = Some(true);
    blank.backup.root = Some(backup_root.clone());
    blank.request_quit();
    drop(blank);

    let session = winc_config::Session {
        folder: folder.clone(),
        tabs: vec![winc_config::Tab::File(selected.clone())],
        active: Some(0),
        ..Default::default()
    };
    let state_path = root.join("state.json");
    let mut editor = Editor::default();
    editor.open_folder(&folder).unwrap();
    let restored = crate::app::restore_at_launch(&mut editor, Some(&session), Some(&backup_root));
    let mut project = test_app();
    project.editor = editor;
    project.state_path = Some(state_path.clone());
    project.adopt_restored(restored);
    let mut winc = Winc::new(
        None,
        Some(Template {
            config_path: None,
            state_path: Some(state_path.clone()),
            backup_root: Some(backup_root.clone()),
        }),
    );
    let key = winc.add_detached(project);
    winc.restore_empty_project();
    assert_eq!(
        winc.app(key).editor.active().unwrap().path.as_deref(),
        Some(selected.as_path())
    );
    winc.app(key).prepare_exit();
    winc.remember_windows();
    drop(winc);
    let state = State::load_from(&state_path);
    let mut editor = Editor::default();
    let session = state.last_session().unwrap();
    editor.open_folder(&session.folder).unwrap();
    let _restored = crate::app::restore_at_launch(&mut editor, Some(session), Some(&backup_root));
    assert_eq!(
        editor.active().unwrap().path.as_deref(),
        Some(selected.as_path())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_backups_are_not_recovered_while_another_instance_holds_the_lock() {
    let root = test_folder("recovery-locked");
    let backup_root = root.join("backups");
    let mut blank = dirty_app();
    blank.config.hot_exit = Some(true);
    blank.backup.root = Some(backup_root.clone());
    blank.request_quit();
    assert!(blank.quit_requested);

    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let mut project = test_app();
    project.editor.open_folder(&folder).unwrap();
    let mut winc = Winc::new(
        None,
        Some(Template {
            config_path: None,
            state_path: None,
            backup_root: Some(backup_root),
        }),
    );
    let active = winc.add_detached(project);
    winc.restore_empty_project();
    assert_eq!(winc.keys(), [active]);
    drop(blank);
    winc.restore_empty_project();
    assert_eq!(winc.keys().len(), 2);
    assert_eq!(winc.active_key(), Some(active));
    let recovered = winc.windows.iter().find(|slot| slot.key != active).unwrap();
    let doc = recovered.app.editor.active().unwrap();
    assert_eq!(doc.text().rope().unwrap().to_string(), "draft");
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
}

use super::*;
use crate::app::terminal::shells::ShellChoice;
use crate::winc::{Envelope, Winc, WindowProxy};
use std::time::Duration;
use winit::event_loop::EventLoop;
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::platform::windows::EventLoopBuilderExtWindows;

struct Harness {
    winc: Winc,
    events: usize,
    create_window: bool,
}

impl ApplicationHandler<Envelope> for Harness {
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Envelope) {
        self.events += 1;
        if std::mem::take(&mut self.create_window) {
            self.winc.resumed(event_loop);
        }
        self.winc.user_event(event_loop, event);
    }
}

fn offscreen_app(proxy: Option<WindowProxy>) -> App {
    let mut app = App::new(Editor::default(), proxy, Bench::off(), Config::default());
    app.renderer =
        Some(Renderer::offscreen(640.0, 420.0, 1.0, app.palette.frame_bg, None, None).unwrap());
    app.measure_fonts();
    app.relayout();
    app
}

#[test]
fn traffic_lights_follow_the_active_project_after_switch_and_close() {
    let mut builder = EventLoop::<Envelope>::with_user_event();
    builder.with_any_thread(true).with_dpi_aware(false);
    let mut event_loop = builder.build().unwrap();
    let mut winc = Winc::new(Some(event_loop.create_proxy()), None);

    let (first, first_proxy) = winc.reserve();
    let first_proxy = first_proxy.unwrap();
    winc.add(first, offscreen_app(Some(first_proxy.clone())));
    let moved = traffic_lights_moved(winc.app(first).proxy.as_ref());

    let (second, second_proxy) = winc.reserve();
    winc.add(second, offscreen_app(second_proxy));
    moved();
    winc.activate_for_test(second);
    winc.app(first).dirty = false;
    winc.app(second).dirty = false;
    assert_eq!(winc.app(second).layout.projects.w, 0.0);

    let mut harness = Harness {
        winc,
        events: 0,
        create_window: false,
    };
    event_loop.pump_app_events(Some(Duration::ZERO), &mut harness);
    assert_eq!(harness.events, 1);
    assert!(!harness.winc.app(first).dirty);
    assert!(harness.winc.app(second).dirty);
    assert!(harness.winc.app(second).layout.projects.w > 0.0);

    let shell = ShellChoice {
        name: "first".into(),
        command: "first".into(),
    };
    first_proxy
        .send_event(UserEvent::TerminalShells(vec![shell.clone()]))
        .unwrap();
    event_loop.pump_app_events(Some(Duration::ZERO), &mut harness);
    assert_eq!(harness.events, 2);
    assert_eq!(
        harness.winc.app(first).terminal.shells.as_slice(),
        std::slice::from_ref(&shell)
    );
    assert!(harness.winc.app(second).terminal.shells.is_empty());

    harness.winc.app(first).request_close_project();
    harness.winc.settle_for_test();
    assert_eq!(harness.winc.keys(), [second]);
    harness.winc.app(second).dirty = false;
    first_proxy
        .send_event(UserEvent::TerminalShells(vec![shell]))
        .unwrap();
    moved();
    event_loop.pump_app_events(Some(Duration::ZERO), &mut harness);
    assert_eq!(harness.events, 4);
    assert!(harness.winc.app(second).terminal.shells.is_empty());
    assert!(harness.winc.app(second).dirty);
    assert_eq!(harness.winc.app(second).layout.projects.w, 0.0);

    background_confirmations_follow_the_window(&mut event_loop, &mut harness, second);
}

fn pump_until(
    event_loop: &mut EventLoop<Envelope>,
    harness: &mut Harness,
    mut done: impl FnMut(&mut Harness) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(harness) {
        assert!(Instant::now() < deadline, "background event timed out");
        event_loop.pump_app_events(Some(Duration::from_millis(20)), harness);
    }
}

fn background_confirmations_follow_the_window(
    event_loop: &mut EventLoop<Envelope>,
    harness: &mut Harness,
    other: crate::winc::WindowKey,
) {
    use crate::app::{AlertButton, PendingUi};
    use winit::keyboard::{Key, NamedKey};

    let root = std::env::temp_dir().join(format!("winc-ui-background-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let git = |args: &[&str]| {
        let result = crate::app::snapshot::quiet(std::process::Command::new("git"))
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Test"]);
    git(&["config", "user.email", "test@example.com"]);
    let file = root.join("main.rs");
    std::fs::write(&file, "saved\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "initial"]);
    let linked = root.with_file_name(format!("winc-ui-linked-{}", std::process::id()));
    git(&["worktree", "add", "-qb", "linked", linked.to_str().unwrap()]);
    let root = winc_config::files::canonicalize(&root).unwrap();
    let linked = winc_config::files::canonicalize(linked).unwrap();

    let other_proxy = harness.winc.app(other).proxy.clone().unwrap();
    harness.create_window = true;
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| h.winc.app(other).has_shell());

    let (owner, proxy) = harness.winc.reserve();
    let proxy = proxy.unwrap();
    let mut editor = Editor::default();
    editor.open_folder(&root).unwrap();
    editor.open(&file);
    let mut app = App::new(editor, Some(proxy.clone()), Bench::off(), Config::default());
    app.config.hot_exit = Some(false);
    app.git.root = Some(root.clone());
    app.git.worktrees = vec![
        winc_editor::git::Worktree {
            path: root.clone(),
            head: None,
            locked: false,
            prunable: false,
        },
        winc_editor::git::Worktree {
            path: linked.clone(),
            head: Some(winc_editor::git::Head::Branch("linked".into())),
            locked: false,
            prunable: false,
        },
    ];
    harness.winc.add(owner, app);
    harness.winc.activate_for_test(owner);
    harness.winc.app(owner).remove_worktree(1);
    harness.winc.activate_for_test(other);
    assert!(harness.winc.app(other).show_alert(
        "other project",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    pump_until(event_loop, harness, |h| {
        h.winc
            .app(owner)
            .pending_ui
            .iter()
            .any(|p| matches!(p, PendingUi::RemoveWorktree(..)))
    });
    assert!(linked.exists());
    assert_eq!(harness.winc.active_for_test(), Some(other));
    assert!(harness.winc.app(other).alert.is_some());
    harness
        .winc
        .app(other)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.active_for_test() == Some(owner) && h.winc.app(owner).alert.is_some()
    });
    assert!(linked.exists());
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    assert!(linked.exists());

    std::fs::create_dir_all(root.join(".winc")).unwrap();
    std::fs::write(
        root.join(".winc/config.json"),
        r#"{"format":{"onSave":true}}"#,
    )
    .unwrap();
    let app = harness.winc.app(owner);
    app.format.formatter = winc_editor::format::Formatter::with_vars(std::env::vars_os().collect());
    app.config.fix_on_save = Some(true);
    app.config
        .fix_commands
        .insert("rust".into(), "cmd.exe /D /C echo fixed".into());
    let doc = app.editor.active_mut().unwrap();
    doc.execute(Command::Insert("draft ".into()), 1, 4);
    let id = doc.id;
    app.save_by_id(id);
    assert!(app.format_save_pending());
    harness.winc.activate_for_test(other);
    assert!(harness.winc.app(other).show_alert(
        "other project",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    pump_until(event_loop, harness, |h| {
        h.winc
            .app(owner)
            .pending_ui
            .iter()
            .any(|p| matches!(p, PendingUi::Format { .. }))
    });
    let approval = root.join(".winc/formatOnSave");
    assert_eq!(harness.winc.app(owner).state.tool_choice(&approval), None);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
    harness
        .winc
        .app(other)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.active_for_test() == Some(owner) && h.winc.app(owner).alert.is_some()
    });
    assert_eq!(harness.winc.app(owner).state.tool_choice(&approval), None);
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    assert_eq!(
        harness.winc.app(owner).state.tool_choice(&approval),
        Some(false)
    );
    assert!(!harness.winc.app(owner).format_save_pending());

    let app = harness.winc.app(owner);
    app.editor
        .document_mut(id)
        .unwrap()
        .execute(Command::Insert("new ".into()), 1, 4);
    let mut terminal = crate::app::terminal::TermTab::new(1, Default::default());
    terminal.process = "vim".into();
    app.terminal.tabs.push(terminal);
    app.save_by_id(id);
    app.request_close_project();
    harness.winc.activate_for_test(other);
    assert!(harness.winc.app(other).show_alert(
        "other project",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    pump_until(event_loop, harness, |h| {
        h.winc
            .app(owner)
            .pending_ui
            .iter()
            .any(|p| matches!(p, PendingUi::Close(_)))
    });
    assert!(harness.winc.app(owner).editor.document(id).is_some());
    harness
        .winc
        .app(other)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.active_for_test() == Some(owner) && h.winc.app(owner).alert.is_some()
    });
    assert_eq!(
        harness.winc.app(owner).closing.confirming,
        Some(CloseRequest::CloseProject)
    );
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    assert!(harness.winc.app(owner).editor.document(id).is_some());
    assert!(harness.winc.app(owner).terminal_busy());
    assert!(harness.winc.app(other).editor.documents.is_empty());

    let app = harness.winc.app(owner);
    app.editor
        .document_mut(id)
        .unwrap()
        .execute(Command::Insert("quit ".into()), 1, 4);
    app.save_by_id(id);
    app.request_quit();
    harness.winc.activate_for_test(other);
    assert!(harness.winc.app(other).show_alert(
        "other project",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    pump_until(event_loop, harness, |h| {
        h.winc
            .app(owner)
            .pending_ui
            .iter()
            .any(|p| matches!(p, PendingUi::Close(CloseRequest::Quit)))
    });
    assert!(!harness.winc.app(owner).closing.quit_requested);
    harness
        .winc
        .app(other)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.active_for_test() == Some(owner) && h.winc.app(owner).alert.is_some()
    });
    assert_eq!(
        harness.winc.app(owner).closing.confirming,
        Some(CloseRequest::Quit)
    );
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    assert!(!harness.winc.app(owner).closing.quit_requested);
    assert!(harness.winc.app(owner).terminal_busy());

    let app = harness.winc.app(owner);
    app.terminal.tabs.clear();
    app.editor.new_untitled();
    let first = app.editor.active_mut().unwrap();
    first.execute(Command::Insert("first draft".into()), 1, 4);
    let first = first.id;
    let held_proxy = app.proxy.take();
    app.pick(Pick::SaveAs(first));
    app.proxy = held_proxy;
    app.editor.new_untitled();
    let remaining = app.editor.active_mut().unwrap();
    remaining.execute(Command::Insert("second draft".into()), 1, 4);
    let remaining = remaining.id;
    app.request_close_project();
    harness.winc.activate_for_test(other);
    assert!(harness.winc.app(other).show_alert(
        "other project",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    let saved_as = root.join("first.txt");
    proxy
        .send_event(UserEvent::Picked(
            Pick::SaveAs(first),
            Some(saved_as.clone()),
        ))
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc
            .app(owner)
            .pending_ui
            .iter()
            .any(|p| matches!(p, PendingUi::Close(_)))
    });
    assert_eq!(std::fs::read_to_string(saved_as).unwrap(), "first draft");
    assert!(harness.winc.app(owner).closing.prompt.is_none());
    assert_eq!(harness.winc.active_for_test(), Some(other));
    harness
        .winc
        .app(other)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    other_proxy
        .send_event(UserEvent::TrafficLightsMoved)
        .unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.active_for_test() == Some(owner)
            && h.winc.app(owner).closing.prompt.is_some()
            && h.winc.app(owner).alert.is_some()
    });
    assert_eq!(
        harness
            .winc
            .app(owner)
            .closing
            .prompt
            .as_ref()
            .unwrap()
            .request,
        CloseRequest::CloseProject
    );
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    assert!(
        harness
            .winc
            .app(owner)
            .editor
            .document(remaining)
            .unwrap()
            .is_dirty()
    );
    assert!(!harness.winc.app(owner).project_closed);
    assert!(harness.winc.app(other).editor.documents.is_empty());

    assert!(harness.winc.app(owner).show_alert(
        "other operation",
        "",
        vec![AlertButton::cancel("Cancel")],
        None,
        |_, _| None,
    ));
    harness.winc.app(other).request_quit();
    proxy.send_event(UserEvent::TrafficLightsMoved).unwrap();
    let previous_events = harness.events;
    pump_until(event_loop, harness, |h| h.events > previous_events);
    assert_eq!(harness.winc.active_for_test(), Some(owner));
    assert!(harness.winc.app(owner).alert.is_some());
    assert!(harness.winc.app(owner).closing.prompt.is_none());
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    proxy.send_event(UserEvent::TrafficLightsMoved).unwrap();
    pump_until(event_loop, harness, |h| {
        h.winc.app(owner).closing.prompt.is_some()
    });
    harness
        .winc
        .app(owner)
        .alert_key(&Key::Named(NamedKey::Escape), false);
    proxy.send_event(UserEvent::TrafficLightsMoved).unwrap();
    let previous_events = harness.events;
    pump_until(event_loop, harness, |h| h.events > previous_events);
    assert!(!harness.winc.app(owner).closing.quit_requested);
    assert!(!harness.winc.app(other).closing.quit_requested);
    assert!(
        harness
            .winc
            .app(owner)
            .editor
            .document(remaining)
            .unwrap()
            .is_dirty()
    );

    let owner_app = harness.winc.app(owner);
    owner_app.cancel_all_formatting();
    owner_app.watcher = None;
    for dir in [&linked, &root] {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match std::fs::remove_dir_all(dir) {
                Ok(()) => break,
                Err(_) if Instant::now() < deadline => {
                    event_loop.pump_app_events(Some(Duration::from_millis(20)), harness);
                }
                Err(error) => panic!("could not remove test directory: {error}"),
            }
        }
    }
}

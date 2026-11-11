//! 项目交接和隐藏窗口时的终端焦点状态。

use super::*;

#[test]
fn handoff_and_hiding_clear_terminal_focus_without_removing_tabs() {
    let mut app = App::new(
        winc_editor::Editor::default(),
        None,
        crate::bench::Bench::off(),
        winc_config::Config::default(),
    );
    app.terminal.tabs.push(TermTab::new(1, Size::default()));
    app.terminal.open = true;
    app.terminal.focus_sent = true;
    app.window.focused = true;

    let state = app.take_shell_state();
    assert!(!app.terminal.focus_sent && !app.window.focused);
    assert_eq!(app.terminal.tabs.len(), 1);

    app.put_shell_state(state, false);
    app.terminal.focus_sent = true;
    app.window.focused = true;
    app.request_close(CloseRequest::CloseWindow);
    assert!(!app.terminal.focus_sent && !app.window.focused);
    assert_eq!(app.terminal.tabs.len(), 1);
}

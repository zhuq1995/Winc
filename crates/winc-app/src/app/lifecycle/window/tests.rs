//! 项目交接窗口状态的测试。

use super::*;
use winc_config::WindowFrame;

fn app() -> App {
    App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    )
}

#[test]
fn shell_handoff_preserves_frame_controls_modifiers_and_clears_preedit() {
    let mut first = app();
    let frame = WindowFrame {
        position: Some((321.0, 123.0)),
        width: 1010.0,
        height: 720.0,
        scale: 1.5,
        maximized: false,
    };
    first.window_frame = Some(frame);
    first.window_controls = true;
    first.modifiers = ModifiersState::SHIFT | ModifiersState::ALT;
    first.option_side = (false, true);
    first.mouse = (30.0, 40.0);
    first.cursor_inside = true;
    first.cursor_icon = CursorIcon::Text;
    first.preedit = "组合".into();
    first.field_preedit = "输入".into();

    let state = first.take_shell_state();
    assert!(!first.window_focused);
    assert!(!first.cursor_inside);
    assert!(first.preedit.is_empty() && first.field_preedit.is_empty());

    let mut second = app();
    second.preedit = "旧组字".into();
    second.field_preedit = "旧输入".into();
    second.put_shell_state(state, true);
    assert_eq!(second.window_frame, Some(frame));
    assert!(second.window_controls);
    assert_eq!(
        second.modifiers,
        ModifiersState::SHIFT | ModifiersState::ALT
    );
    assert_eq!(second.option_side, (false, true));
    assert_eq!(second.mouse, (30.0, 40.0));
    assert!(second.cursor_inside && second.window_focused);
    assert_eq!(second.cursor_icon, CursorIcon::Text);
    assert!(second.preedit.is_empty() && second.field_preedit.is_empty());

    second.modifiers = ModifiersState::empty();
    second.option_side = (false, false);
    let state = second.take_shell_state();
    first.put_shell_state(state, false);
    assert_eq!(first.window_frame, Some(frame));
    assert_eq!(first.modifiers, ModifiersState::empty());
    assert_eq!(first.option_side, (false, false));
    assert!(!first.window_focused);
}

#[test]
fn shell_handoff_applies_current_appearance_to_the_configured_named_theme() {
    let mut first = app();
    let mut second = app();
    let light = &crate::theme::themes(false)[1];
    let dark = &crate::theme::themes(true)[1];
    second.config.light_theme = Some(light.id.to_owned());
    second.config.dark_theme = Some(dark.id.to_owned());
    second.config.theme = Some(ThemeChoice::Auto);
    second.theme_choice = ThemeChoice::Auto;

    first.set_theme(Theme::Light);
    second.put_shell_state(first.take_shell_state(), true);
    assert!(std::ptr::eq(second.palette, light.palette));
    assert_eq!(second.theme_choice, ThemeChoice::Auto);
    assert_eq!(second.config.light_theme.as_deref(), Some(light.id));

    // 系统切换外观后，重新接上窗口的项目使用当前外观和各自的主题配置。
    second.set_theme(Theme::Dark);
    let state = second.take_shell_state();
    first.put_shell_state(state, true);
    second.put_shell_state(first.take_shell_state(), true);
    assert!(std::ptr::eq(second.palette, dark.palette));
    assert_eq!(second.theme_choice, ThemeChoice::Auto);
    assert_eq!(second.config.dark_theme.as_deref(), Some(dark.id));

    first.theme_choice = ThemeChoice::Dark;
    first.set_theme(Theme::Dark);
    second.config.theme = Some(ThemeChoice::Light);
    second.theme_choice = ThemeChoice::Light;
    second.put_shell_state(first.take_shell_state(), true);
    assert!(std::ptr::eq(second.palette, light.palette));
    assert_eq!(second.theme_choice, ThemeChoice::Light);
    assert_eq!(second.config.theme, Some(ThemeChoice::Light));

    first.theme_choice = ThemeChoice::Light;
    first.set_theme(Theme::Light);
    second.config.theme = Some(ThemeChoice::Dark);
    second.theme_choice = ThemeChoice::Dark;
    second.put_shell_state(first.take_shell_state(), true);
    assert!(std::ptr::eq(second.palette, dark.palette));
    assert_eq!(second.theme_choice, ThemeChoice::Dark);
}

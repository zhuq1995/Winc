//! 窗口交接后，通过已安装的 Win32 回调查询当前项目的控件区域。

#![allow(unsafe_code)] // 测试窗口的坐标转换与同步命中查询。

use super::*;
use std::sync::Arc;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{HTCLIENT, SendMessageW, WM_NCHITTEST};
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::platform::windows::{EventLoopBuilderExtWindows, WindowExtWindows};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

fn native_hit(window: &Window, zoom: f32, rect: Rect) -> u32 {
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        unreachable!()
    };
    let hwnd = handle.hwnd.get() as _;
    // SAFETY: 窗口在本线程创建且仍存活，point 指向可写的栈变量。
    unsafe {
        let scale = GetDpiForWindow(hwnd) as f32 / 96.0 * zoom;
        let mut point = POINT {
            x: ((rect.x + rect.w / 2.0) * scale).round() as i32,
            y: ((rect.y + rect.h / 2.0) * scale).round() as i32,
        };
        assert_ne!(ClientToScreen(hwnd, &mut point), 0);
        let position = (point.x as u16 as u32 | ((point.y as u16 as u32) << 16)) as isize;
        SendMessageW(hwnd, WM_NCHITTEST, 0, position) as u32
    }
}

fn verify_controls(app: &mut App) {
    paint(app);
    app.sync_window_input();
    let window = app.window.handle.as_ref().unwrap();
    assert_eq!(
        native_hit(window, app.zoom(), app.tabs.new_tab),
        HTCLIENT,
        "+"
    );
    for hit in &app.hits.tabs {
        if app.tabs.view.contains(
            hit.close.x + hit.close.w / 2.0,
            hit.close.y + hit.close.h / 2.0,
        ) {
            assert_eq!(native_hit(window, app.zoom(), hit.close), HTCLIENT, "close");
        }
    }
    for (rect, view) in app.sidebar_tabs() {
        assert_eq!(native_hit(window, app.zoom(), rect), HTCLIENT, "{view:?}");
    }
}

#[test]
#[allow(deprecated)] // Windows 支持在事件循环运行前创建测试窗口。
fn project_switch_transfers_native_hit_test_updates() {
    let event_loop = EventLoop::builder().with_any_thread(true).build().unwrap();
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Winc hit test regression")
                    .with_decorations(false)
                    .with_visible(false)
                    .with_active(false)
                    .with_inner_size(LogicalSize::new(SIZE.0, SIZE.1)),
            )
            .unwrap(),
    );
    // 禁用测试窗口，交接中的显示与焦点调用保持用户当前窗口的输入焦点。
    window.set_enable(false);
    let dir = search_folder("windows-native-launch");
    let mut a = blank_app(false);
    let mut b = blank_app(false);
    b.open_folder_now(&dir);
    a.panels.sidebar.visible = true;
    b.panels.sidebar.visible = true;
    b.git.root = Some(std::env::temp_dir());
    a.new_file();
    b.new_file();
    a.window.handle = Some(window.clone());
    a.touch.window_input = crate::windows::WindowInput::new(&window);
    assert!(a.touch.window_input.is_some());
    a.panels.sidebar.width = 300.0;
    verify_controls(&mut a);

    let shell = a.take_window().unwrap();
    assert!(a.touch.window_input.is_none());
    b.put_window(shell);
    window.set_visible(false);
    assert!(b.touch.window_input.is_some());
    for width in [700.0, MIN_PANEL_WIDTH, 500.0] {
        b.panels.sidebar.width = width;
        verify_controls(&mut b);
    }

    let shell = b.take_window().unwrap();
    assert!(b.touch.window_input.is_none());
    a.put_window(shell);
    window.set_visible(false);
    assert!(a.touch.window_input.is_some());
    a.panels.sidebar.width = 600.0;
    verify_controls(&mut a);

    let mut winc = normal_winc();
    let first = winc.add_detached(a);
    let second = winc.add_detached(b);
    let file = winc.app(second).editor.root().unwrap().join("src/main.rs");

    // IPC 把文件交给后台项目，同时移交原生窗口。
    assert!(winc.launched_for_test(vec![target(&file)]));
    assert_eq!(winc.active_for_test(), Some(second));
    assert!(winc.app(first).window.handle.is_none());
    let app = winc.app(second);
    assert!(Arc::ptr_eq(app.window.handle.as_ref().unwrap(), &window));
    assert_eq!(app.editor.active().unwrap().name(), "main.rs");
    verify_controls(app);
    winc.apply_request_for_test(Request::Focus(first));
    assert!(Arc::ptr_eq(
        winc.app(first).window.handle.as_ref().unwrap(),
        &window
    ));

    // 全应用退出时，后台项目的保存提示使用同一个原生窗口。
    let app = winc.app(second);
    app.activate_tab(0);
    app.config.hot_exit = Some(false);
    app.run(Command::Insert("background note".into()));
    winc.app(first).request_close_window();
    assert!(!winc.settle_for_test().1);
    winc.apply_request_for_test(Request::Focus(first));
    assert!(!winc.launched_for_test(vec![target(&dir)]));
    assert_eq!(winc.active_for_test(), Some(second));
    assert!(winc.app(first).window.handle.is_none());
    let app = winc.app(second);
    assert!(Arc::ptr_eq(app.window.handle.as_ref().unwrap(), &window));
    assert!(app.closing.prompt.is_some());
    paint(app);
    let alert = app.overlays.alert.as_ref().expect("保存确认在当前窗口绘制");
    assert!(!alert.parts.buttons.is_empty());
    app.overlays.alert = None;
    app.answer_save_prompt(SaveChoice::Cancel);
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(first).closing.quit_requested);
    assert!(!winc.app(second).closing.quit_requested);
    window.set_visible(false);
    let _ = std::fs::remove_dir_all(&dir);
}

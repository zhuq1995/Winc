//! 临时打开模式的测试：关闭标准输入标签后停止读取。

use super::*;
use crate::bench::Bench;

/// 测试中标准输入标签的标题：测试以源语言运行，与 [`stdin_title`] 的结果相同。
const STDIN_TITLE: &str = "标准输入";

#[test]
fn closing_the_stdin_tab_stops_reading() {
    let mut app = App::new(
        Editor::default(),
        None,
        Bench::from_env(),
        Config::default(),
    );
    let index = app.editor.show_output(STDIN_TITLE, "");
    let id = app.editor.documents[index].id;
    let (send, receive) = mpsc::channel();
    app.one_off.stdin = Some((id, receive));

    send.send(Piece::Text("one\n".to_owned())).unwrap();
    app.poll_stdin();
    assert_eq!(app.editor.documents[0].text().line_count(), 2);

    // 关闭标签后 `poll_stdin` 丢弃接收端，发送端再发送会返回错误，后台线程据此退出。
    app.remove_tabs(&[id]);
    let _ = send.send(Piece::Text("two\n".to_owned()));
    app.poll_stdin();
    assert!(app.one_off.stdin.is_none());
    assert!(send.send(Piece::Text("three\n".to_owned())).is_err());
}

#[test]
fn opening_a_folder_keeps_the_stdin_stream_in_the_one_off_session() {
    let folder = std::env::temp_dir().join(format!("winc-stdin-folder-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let folder = winc_config::files::canonicalize(folder).unwrap();
    let mut app = App::new(
        Editor::default(),
        None,
        Bench::from_env(),
        Config::default(),
    );
    let index = app.editor.show_output(STDIN_TITLE, "");
    let id = app.editor.documents[index].id;
    let (send, receive) = mpsc::channel();
    app.one_off.stdin = Some((id, receive));
    let mut winc = crate::winc::Winc::new(None, None);
    let key = winc.add_detached(app);

    winc.app(key).open_folder(&folder);
    let (requests, exit) = winc.settle_for_test();
    assert!(!exit);
    assert_eq!(requests, [crate::winc::Request::OpenFolder(folder.clone())]);
    assert!(winc.open_folder_for_test(&folder));
    assert_eq!(winc.app(key).editor.root(), Some(folder.as_path()));
    assert_eq!(winc.keys(), [key]);

    send.send(Piece::Text("still reading\n".to_owned()))
        .unwrap();
    winc.app(key).poll_stdin();
    let doc = winc.app(key).editor.document(id).unwrap();
    assert_eq!(doc.text().rope().unwrap().to_string(), "still reading\n");
    assert!(winc.app(key).one_off.stdin.is_some());
    drop(winc);
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn switching_folders_in_a_wait_session_preserves_the_waited_tab_and_cancel() {
    let root = std::env::temp_dir().join(format!("winc-wait-folders-{}", std::process::id()));
    let first = root.join("first");
    let second = root.join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let first = winc_config::files::canonicalize(first).unwrap();
    let second = winc_config::files::canonicalize(second).unwrap();

    let mut app = App::new(
        Editor::default(),
        None,
        Bench::from_env(),
        Config::default(),
    );
    app.editor.new_untitled();
    app.editor
        .active_mut()
        .unwrap()
        .execute(winc_editor::Command::Insert("draft".into()), 1, 4);
    app.start_one_off(&Args {
        wait: true,
        ..Default::default()
    });
    let id = app.editor.active().unwrap().id;
    let mut winc = crate::winc::Winc::new(None, None);
    let key = winc.add_detached(app);

    winc.app(key).open_folder(&first);
    let (requests, exit) = winc.settle_for_test();
    assert!(!exit);
    assert_eq!(requests, [crate::winc::Request::OpenFolder(first.clone())]);
    assert!(winc.open_folder_for_test(&first));
    assert_eq!(winc.app(key).editor.root(), Some(first.as_path()));
    assert!(winc.app(key).editor.document(id).unwrap().is_dirty());
    assert!(
        winc.app(key)
            .one_off
            .waiting
            .as_ref()
            .unwrap()
            .contains(&id)
    );

    winc.app(key).open_folder(&second);
    assert_eq!(
        winc.settle_for_test().0,
        [crate::winc::Request::OpenFolder(second.clone())]
    );
    assert!(winc.open_folder_for_test(&second));
    assert!(winc.app(key).closing.prompt.is_some());
    winc.app(key).answer_save_prompt(SaveChoice::Cancel);
    assert_eq!(winc.app(key).editor.root(), Some(first.as_path()));
    assert!(winc.app(key).editor.document(id).unwrap().is_dirty());
    assert!(
        winc.app(key)
            .one_off
            .waiting
            .as_ref()
            .unwrap()
            .contains(&id)
    );
    assert!(!winc.settle_for_test().1);

    assert!(winc.open_folder_for_test(&second));
    winc.app(key).answer_save_prompt(SaveChoice::Discard);
    assert_eq!(winc.app(key).editor.root(), Some(second.as_path()));
    assert!(winc.app(key).editor.document(id).is_none());
    assert!(winc.settle_for_test().1);
    assert_eq!(winc.keys(), [key]);
    drop(winc);
    let _ = std::fs::remove_dir_all(root);
}

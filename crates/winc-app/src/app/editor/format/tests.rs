//! 格式化的测试：后台结果的收取与保存前的步骤。

use super::*;

impl App {
    /// 等待后台格式化送回一个结果并处理，最长等待 20 秒；没有正在进行的格式化时立即返回。
    ///
    /// 测试中没有事件循环。
    pub(in crate::app) fn wait_formatting(&mut self) {
        if let Ok((id, job, result)) = self.format.receiver.recv_timeout(Duration::from_secs(20)) {
            self.formatting_done(id, job, result);
        }
    }

    /// 保存文档 `id` 时是否先格式化。
    #[cfg(unix)]
    pub(in crate::app) fn formats_on_save(&self, id: DocumentId) -> bool {
        self.on_save(Task::Format, id)
    }

    #[cfg(unix)]
    pub(in crate::app) fn save_id_for_test(&mut self, id: DocumentId) -> u64 {
        let save_id = self.format.next_id();
        self.format.saving.insert(id, save_id);
        save_id
    }

    /// 返回正在后台格式化的文件数。
    pub(in crate::app) fn formatting_count(&self) -> usize {
        self.format.running.len()
    }
}

fn sample(name: &str) -> (PathBuf, PathBuf, App, DocumentId) {
    let root =
        std::env::temp_dir().join(format!("winc-format-close-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("main.rs");
    std::fs::write(&file, "saved\n").unwrap();
    let mut editor = Editor::default();
    editor.open_folder(&root).unwrap();
    editor.open(&file);
    let doc = editor.active_mut().unwrap();
    doc.execute(Command::Insert("draft ".into()), 1, 4);
    let id = doc.id;
    let mut app = App::new(editor, None, crate::bench::Bench::off(), Config::default());
    app.config.hot_exit = Some(false);
    (root, file, app, id)
}

fn manual_job(app: &mut App, id: DocumentId) -> u64 {
    let sent = app.editor.document(id).unwrap().format_source().unwrap();
    let job = app.format.next_id();
    app.format.running.push(Running {
        id,
        job,
        task: Task::Format,
        sent,
        save: false,
        partial: false,
        started: Instant::now(),
        shown: false,
        cancel: Cancel::default(),
    });
    job
}

fn saving_job(app: &mut App, id: DocumentId) -> u64 {
    let job = manual_job(app, id);
    app.config.format_on_save = Some(true);
    app.save_by_id(id);
    assert!(app.format_save_pending());
    job
}

fn complete(app: &mut App, id: DocumentId, job: u64, result: Result<Formatted, FormatError>) {
    app.format.sender.send((id, job, result)).unwrap();
    app.wait_formatting();
}

fn formatted() -> Result<Formatted, FormatError> {
    Ok(Formatted {
        changes: vec![Change {
            start: 0,
            removed: "draft ".into(),
            inserted: "DRAFT ".into(),
        }],
        lines: 1,
        tool: "test".into(),
    })
}

#[test]
fn real_fix_and_format_run_before_deferred_close() {
    let (root, file, mut app, id) = sample("real-tool");
    app.format.formatter = Formatter::with_vars(std::env::vars_os().collect());
    #[cfg(windows)]
    let command = "cmd.exe /D /C echo DRAFT saved";
    #[cfg(unix)]
    let command = "/bin/sh -c 'echo DRAFT saved'";
    app.config.fix_on_save = Some(true);
    app.config.format_on_save = Some(true);
    app.config
        .fix_commands
        .insert("rust".into(), command.into());
    app.config
        .format_commands
        .insert("rust".into(), command.into());
    app.save_by_id(id);
    assert_eq!(app.formatting_count(), 1);
    app.request_close_project();
    assert!(app.editor.document(id).is_some());
    app.wait_formatting();
    assert_eq!(app.formatting_count(), 1);
    assert!(app.editor.document(id).is_some());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
    app.wait_formatting();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "DRAFT saved\n");
    assert!(app.editor.document(id).is_none());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_format_save_finishes_before_project_close_even_with_hot_exit() {
    let (root, file, mut app, id) = sample("project");
    app.config.hot_exit = Some(true);
    app.backup.root = Some(root.join("backups"));
    let job = saving_job(&mut app, id);
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(app);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.app(first).request_close_project();
    assert!(!winc.settle_for_test().1);
    assert_eq!(winc.keys(), [first, second]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
    assert!(winc.app(first).closing.prompt.is_none());

    complete(winc.app(first), id, job, formatted());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "DRAFT saved\n");
    assert!(!winc.settle_for_test().1);
    assert_eq!(winc.keys(), [second]);
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn global_quit_waits_for_format_save_and_saves_when_format_fails_or_is_cancelled() {
    for outcome in ["success", "failure", "cancel"] {
        let (root, file, mut app, id) = sample(outcome);
        let job = saving_job(&mut app, id);
        let mut winc = crate::winc::Winc::new(None, None);
        let first = winc.add_detached(app);
        let second = winc.add_detached(App::new(
            Editor::default(),
            None,
            crate::bench::Bench::off(),
            Config::default(),
        ));
        winc.app(second).request_quit();
        assert!(!winc.settle_for_test().1);
        assert!(winc.app(first).quit_pending());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
        let result = match outcome {
            "success" => formatted(),
            "failure" => Err(FormatError::Failed("broken".into())),
            _ => Err(FormatError::Cancelled),
        };
        complete(winc.app(first), id, job, result);
        let expected = if outcome == "success" {
            "DRAFT saved\n"
        } else {
            "draft saved\n"
        };
        assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
        assert!(winc.settle_for_test().1);
        drop(winc);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_write_cancels_global_quit_and_keeps_the_dirty_document() {
    let (root, file, mut app, id) = sample("failed-write");
    let job = saving_job(&mut app, id);
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(app);
    let second = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    winc.app(second).request_quit();
    assert!(!winc.settle_for_test().1);
    winc.app(first).editor.document_mut(id).unwrap().path = Some(file.join("target.rs"));
    complete(winc.app(first), id, job, formatted());
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(first).closing.quit_requested && !winc.app(second).closing.quit_requested);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "saved\n");
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_save_after_project_agreed_to_quit_cancels_global_quit() {
    let (root, file, mut saving, id) = sample("queued-failure");
    saving.editor.document_mut(id).unwrap().history.mark_saved();
    let (other_root, _, other, _) = sample("queued-other");
    let mut winc = crate::winc::Winc::new(None, None);
    let first = winc.add_detached(saving);
    let second = winc.add_detached(other);
    winc.app(first).request_quit();
    assert!(!winc.settle_for_test().1);
    assert!(winc.app(second).closing.prompt.is_some());
    winc.app(first)
        .editor
        .document_mut(id)
        .unwrap()
        .execute(Command::Insert("new ".into()), 1, 4);
    let job = saving_job(winc.app(first), id);
    assert!(!winc.settle_for_test().1);
    winc.app(first).editor.document_mut(id).unwrap().path = Some(file.join("target.rs"));
    complete(winc.app(first), id, job, formatted());
    assert!(!winc.settle_for_test().1);
    winc.app(second).answer_save_prompt(SaveChoice::Discard);
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(first).closing.quit_requested && !winc.app(second).closing.quit_requested);
    assert!(winc.app(first).editor.document(id).unwrap().is_dirty());
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(other_root).unwrap();
}

#[test]
fn cancelled_job_and_stale_result_do_not_complete_a_later_save() {
    let (root, file, mut app, id) = sample("stale-job");
    let old = saving_job(&mut app, id);
    app.request_quit();
    app.cancel_format(id);
    assert!(!app.quit_pending());
    let current = saving_job(&mut app, id);
    app.request_quit();
    complete(&mut app, id, old, formatted());
    assert!(app.format_save_pending());
    assert!(!app.closing.quit_requested);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
    app.editor
        .document_mut(id)
        .unwrap()
        .execute(Command::Insert("new ".into()), 1, 4);
    let latest = app
        .editor
        .document(id)
        .unwrap()
        .text()
        .rope()
        .unwrap()
        .to_string();
    complete(&mut app, id, current, formatted());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), latest);
    assert!(app.closing.quit_requested);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_format_saves_finish_before_a_deferred_quit() {
    let (root, first_file, mut app, first_id) = sample("concurrent");
    let second_file = root.join("other.rs");
    std::fs::write(&second_file, "saved\n").unwrap();
    app.editor.open(&second_file);
    let second = app.editor.active_mut().unwrap();
    second.execute(Command::Insert("draft ".into()), 1, 4);
    let second_id = second.id;
    let first_job = saving_job(&mut app, first_id);
    let second_job = saving_job(&mut app, second_id);
    app.request_quit();
    complete(&mut app, second_id, second_job, formatted());
    assert!(!app.closing.quit_requested && app.quit_pending());
    assert_eq!(
        std::fs::read_to_string(&second_file).unwrap(),
        "DRAFT saved\n"
    );
    assert_eq!(std::fs::read_to_string(&first_file).unwrap(), "saved\n");
    complete(&mut app, first_id, first_job, formatted());
    assert_eq!(
        std::fs::read_to_string(first_file).unwrap(),
        "DRAFT saved\n"
    );
    assert!(app.closing.quit_requested);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelling_global_quit_clears_the_deferred_quit_but_keeps_the_save() {
    let (root, file, mut app, id) = sample("cancel-quit");
    let job = saving_job(&mut app, id);
    app.request_quit();
    let mut winc = crate::winc::Winc::new(None, None);
    let saving = winc.add_detached(app);
    let starter = winc.add_detached(App::new(
        Editor::default(),
        None,
        crate::bench::Bench::off(),
        Config::default(),
    ));
    let (_, _, cancelling_app, _) = sample("cancel-other");
    let cancelling = winc.add_detached(cancelling_app);
    winc.app(starter).request_quit();
    assert!(!winc.settle_for_test().1);
    winc.app(cancelling).answer_save_prompt(SaveChoice::Cancel);
    assert!(!winc.settle_for_test().1);
    assert!(!winc.app(saving).quit_pending());
    complete(winc.app(saving), id, job, formatted());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "DRAFT saved\n");
    assert!(!winc.app(saving).closing.quit_requested && !winc.settle_for_test().1);
    drop(winc);
    std::fs::remove_dir_all(root).unwrap();
    let other = std::env::temp_dir().join(format!(
        "winc-format-close-cancel-other-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(other).unwrap();
}

#[test]
fn quit_requested_during_save_prompt_and_format_save_respects_prompt_choice() {
    for cancel in [false, true] {
        for answer_first in [false, true] {
            let (root, file, mut app, id) = sample(&format!("prompt-{cancel}-{answer_first}"));
            app.close_tab(0);
            assert!(app.closing.prompt.is_some());
            let job = saving_job(&mut app, id);
            app.request_quit();
            let choice = if cancel {
                SaveChoice::Cancel
            } else {
                SaveChoice::Discard
            };
            if answer_first {
                app.answer_save_prompt(choice);
                assert!(app.editor.document(id).is_some() && !app.closing.quit_requested);
            }
            complete(&mut app, id, job, formatted());
            if !answer_first {
                assert!(!app.closing.quit_requested);
                app.answer_save_prompt(choice);
            }
            assert_eq!(app.closing.quit_requested, !cancel);
            assert_eq!(app.editor.document(id).is_some(), cancel);
            assert_eq!(std::fs::read_to_string(file).unwrap(), "DRAFT saved\n");
            drop(app);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn format_close_waits_for_an_ordinary_save_as_to_finish() {
    for cancel in [false, true] {
        for picker_first in [false, true] {
            let (root, file, mut app, id) = sample(&format!("picker-{cancel}-{picker_first}"));
            let job = manual_job(&mut app, id);
            app.config.format_on_save = Some(true);
            app.editor.new_untitled();
            let other = app.editor.active_mut().unwrap();
            other.execute(Command::Insert("other".into()), 1, 4);
            let other_id = other.id;
            app.close_tab(0);
            app.answer_save_prompt(SaveChoice::Save);
            assert!(app.format.then_close.is_some());
            app.pick(Pick::SaveAs(other_id));
            if picker_first {
                app.on_picked(
                    Pick::SaveAs(other_id),
                    (!cancel).then(|| root.join("other.txt")),
                );
                assert!(app.editor.document(id).is_some());
            }
            complete(&mut app, id, job, formatted());
            assert_eq!(std::fs::read_to_string(&file).unwrap(), "DRAFT saved\n");
            if !picker_first {
                assert!(app.editor.document(id).is_some());
                assert!(app.save_as_picker.is_some());
                app.on_picked(
                    Pick::SaveAs(other_id),
                    (!cancel).then(|| root.join("other.txt")),
                );
            }
            assert_eq!(app.editor.document(id).is_some(), cancel);
            assert!(!app.closing.quit_requested);
            if cancel {
                assert!(app.editor.document(other_id).unwrap().is_dirty());
            } else {
                assert_eq!(
                    std::fs::read_to_string(root.join("other.txt")).unwrap(),
                    "other"
                );
            }
            drop(app);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn format_save_failure_preserves_an_independent_save_as_continuation() {
    let (root, file, mut app, id) = sample("picker-independent");
    app.editor.new_untitled();
    let other = app.editor.active_mut().unwrap();
    other.execute(Command::Insert("other".into()), 1, 4);
    let other_id = other.id;
    app.close_tab(1);
    app.answer_save_prompt(SaveChoice::Save);
    let job = saving_job(&mut app, id);
    app.editor.document_mut(id).unwrap().path = Some(file.join("invalid"));
    complete(&mut app, id, job, formatted());
    assert_eq!(app.save_as_picker, Some(other_id));
    assert!(app.closing.continuation.is_some());
    app.on_picked(Pick::SaveAs(other_id), Some(root.join("other.txt")));
    assert!(app.editor.document(other_id).is_none());
    assert!(app.editor.document(id).unwrap().is_dirty());
    assert_eq!(
        std::fs::read_to_string(root.join("other.txt")).unwrap(),
        "other"
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_tool_approval_does_not_complete_a_new_save() {
    let (root, file, mut app, id) = sample("approval");
    let old = app.format.next_id();
    app.format.saving.insert(id, old);
    app.request_quit();
    app.cancel_format(id);
    let current = app.format.next_id();
    app.format.saving.insert(id, current);
    app.request_quit();
    let answer = |save_id| ToolAnswer {
        id,
        task: Task::Format,
        save: true,
        save_id: Some(save_id),
        range: None,
        folder: root.clone(),
        run: None,
    };
    app.answer_tool_choice(answer(old));
    assert!(app.format_save_pending() && !app.closing.quit_requested);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
    app.answer_tool_choice(answer(current));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "draft saved\n");
    assert!(app.closing.quit_requested);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn manual_formatting_does_not_block_project_close() {
    let (root, _, mut app, id) = sample("manual");
    app.editor.document_mut(id).unwrap().history.mark_saved();
    manual_job(&mut app, id);
    app.request_close_project();
    assert!(!app.closing_pending());
    assert_eq!(app.formatting_count(), 0);
    assert!(app.editor.document(id).is_none());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

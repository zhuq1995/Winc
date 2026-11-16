//! 格式化的测试：后台结果的收取与保存前的步骤。

use super::*;

impl App {
    /// 等待后台格式化送回一个结果并处理，最长等待 20 秒；没有正在进行的格式化时立即返回。
    ///
    /// 测试中没有事件循环。
    pub(in crate::app) fn wait_formatting(&mut self) {
        if self.format.running.is_empty() {
            return;
        }
        if let Ok((id, job, result)) = self.format.receiver.recv_timeout(Duration::from_secs(20)) {
            self.formatting_done(id, job, result);
        }
    }

    /// 保存文档 `id` 时是否先格式化。
    pub(in crate::app) fn formats_on_save(&self, id: DocumentId) -> bool {
        self.on_save(Task::Format, id)
    }

    /// 测试用：记下一次保存流程的编号；回答保存前的询问时带上它。
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

/// 后台结果带着任务编号回来：编号不是当前任务时丢弃，不写回文档。
#[test]
fn a_stale_format_result_is_discarded() {
    let root = std::env::temp_dir().join(format!("winc-format-stale-{}", std::process::id()));
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

    // 结果带着更早的任务编号回来：丢弃，文档保持原样。
    let stale = Ok(Formatted {
        changes: vec![Change {
            start: 0,
            removed: "draft ".into(),
            inserted: "STALE ".into(),
        }],
        lines: 1,
        tool: "test".into(),
    });
    app.format.sender.send((id, job - 1, stale)).unwrap();
    app.wait_formatting();
    assert_eq!(app.formatting_count(), 1);
    assert_eq!(
        app.editor.document(id).unwrap().text().rope().unwrap(),
        "draft saved\n"
    );

    // 编号对上的结果照常应用。
    let current = Ok(Formatted {
        changes: vec![Change {
            start: 0,
            removed: "draft ".into(),
            inserted: "DRAFT ".into(),
        }],
        lines: 1,
        tool: "test".into(),
    });
    app.format.sender.send((id, job, current)).unwrap();
    app.wait_formatting();
    assert_eq!(app.formatting_count(), 0);
    assert_eq!(
        app.editor.document(id).unwrap().text().rope().unwrap(),
        "DRAFT saved\n"
    );

    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

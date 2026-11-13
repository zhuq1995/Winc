//! 关闭标签、切换目录和退出，以及有未保存修改时的「保存」「不保存」「取消」选择。

use winc_i18n::t;

use crate::app::{AlertButton, AlertKey};

use super::*;

/// 要关闭的对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseRequest {
    /// 指定的几个标签。
    Tabs(Vec<DocumentId>),
    /// 退出：关闭所有标签。
    Quit,
    /// 关闭窗口而不退出应用（macOS）：隐藏窗口，项目和标签都留着。
    CloseWindow,
    /// 关掉这个项目（侧栏条目上的 ✕）：关闭所有标签、目录和终端，窗口让给下一个项目。
    CloseProject,
    /// 切换到另一个目录：关闭所有标签，再打开该目录。
    Switch(PathBuf),
    /// 关闭目录，回到开始页。
    CloseFolder,
}

/// 提示框上被点击的按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveChoice {
    /// 保存修改后再关闭。
    Save,
    /// 放弃修改并关闭。
    Discard,
    /// 取消，不关闭任何内容。
    Cancel,
}

impl SaveChoice {
    /// 提示框按钮的顺序，下标即按钮序号。
    ///
    /// `NSAlert` 的按钮按序号从右向左排列，因此从右向左依次为：保存（`↩`，默认按钮）、
    /// 取消（`Esc`）、不保存（`⌘D`）。
    const BUTTONS: [SaveChoice; 3] = [SaveChoice::Save, SaveChoice::Cancel, SaveChoice::Discard];

    /// 返回按钮序号 `index` 对应的选择；序号超出范围时视为取消。
    fn from_index(index: usize) -> SaveChoice {
        Self::BUTTONS
            .get(index)
            .copied()
            .unwrap_or(SaveChoice::Cancel)
    }
}

/// 正在询问的一次关闭请求：要关闭什么，其中哪些文档有未保存的修改。
#[derive(Debug, Clone)]
pub struct SavePrompt {
    /// 要执行的关闭请求。
    pub request: CloseRequest,
    /// 有未保存修改、需要询问的文档，按保存顺序排列。
    pub dirty: Vec<DocumentId>,
    /// 这次保存和关闭结束后继续处理的一次关闭请求。
    pub(crate) deferred: Option<CloseRequest>,
}

impl App {
    /// 关闭第 `index` 个标签；有未保存的修改时先询问。
    pub(in crate::app) fn close_tab(&mut self, index: usize) {
        if let Some(doc) = self.editor.documents.get(index) {
            self.request_close(CloseRequest::Tabs(vec![doc.id]));
        }
    }

    /// 一次关闭多个标签：没有修改的直接关闭，有修改的合并为一次询问。
    pub(in crate::app) fn close_tabs(&mut self, ids: Vec<DocumentId>) {
        self.request_close(CloseRequest::Tabs(ids));
    }

    /// 处理退出请求：`⌘Q`、程序坞菜单中的「退出」、命令面板中的「退出」。
    ///
    /// 下次启动时打开当前目录（设置中关掉了「启动时打开上次的目录」时除外）。
    pub(crate) fn request_quit(&mut self) {
        if self.closing_pending() {
            self.defer_close(CloseRequest::Quit);
            return;
        }
        if !self.confirm_terminal_quit(CloseRequest::Quit) {
            self.request_close(CloseRequest::Quit);
        }
    }

    /// 处理关闭窗口：标题栏左侧的关闭按钮、`⇧⌘W`，以及没有标签时的 `⌘W`。
    ///
    /// 窗口里放着所有项目。macOS 上关闭窗口不退出应用：隐藏窗口，项目和标签都留着，点击程序坞
    /// 图标原样回来，退出用 ⌘Q；其他平台上关闭窗口就是退出，各项目的未保存内容和终端按退出流程
    /// 逐个确认。
    pub(in crate::app) fn request_close_window(&mut self) {
        if cfg!(target_os = "macos") {
            self.request_close(CloseRequest::CloseWindow);
        } else {
            self.request_quit();
        }
    }

    /// 处理关闭项目（侧栏条目右端的 ✕）：先确认终端里运行中的程序，再关闭标签、目录和终端。
    ///
    /// 还有别的项目时，窗口交给相邻的项目；这是最后一个项目时留在开始页上。
    pub(crate) fn request_close_project(&mut self) {
        if self.closing_pending() {
            self.defer_close(CloseRequest::CloseProject);
            return;
        }
        if !self.confirm_terminal_quit(CloseRequest::CloseProject) {
            self.request_close(CloseRequest::CloseProject);
        }
    }

    /// 退出还没有结果：正在询问、保存或等待终端确认。
    /// 应用层据此判断一次 ⌘Q 是还在进行还是被取消了。
    pub(crate) fn closing_pending(&self) -> bool {
        self.closing.prompt.is_some()
            || self.closing.continuation.is_some()
            || self.save_as_picker.is_some()
            || self.format.then_close.is_some()
            || self.format_save_pending()
            || self.closing.confirming.is_some()
            || self
                .pending_ui
                .iter()
                .any(|action| matches!(action, PendingUi::Save(_) | PendingUi::Close(_)))
    }

    /// 是否正在为退出询问（要不要保存、要不要结束终端里的程序），而不是为别的关闭。
    pub(crate) fn quit_pending(&self) -> bool {
        let quits = |p: &Option<SavePrompt>| {
            p.as_ref().is_some_and(|p| {
                p.request == CloseRequest::Quit || p.deferred == Some(CloseRequest::Quit)
            })
        };
        quits(&self.closing.prompt)
            || quits(&self.closing.continuation)
            || quits(&self.format.then_close)
            || self.deferred_close == Some(CloseRequest::Quit)
            || self.closing.confirming == Some(CloseRequest::Quit)
            || self.pending_ui.iter().any(|action| match action {
                PendingUi::Save(prompt) => {
                    prompt.request == CloseRequest::Quit
                        || prompt.deferred == Some(CloseRequest::Quit)
                }
                PendingUi::Close(request) => *request == CloseRequest::Quit,
                _ => false,
            })
    }

    /// 取消退出意图，保留正在进行的保存和其他关闭操作。
    pub(crate) fn cancel_pending_quit(&mut self) {
        self.closing.quit_requested = false;
        if self.deferred_close == Some(CloseRequest::Quit) {
            self.deferred_close = None;
        }
        // 空标签列表让已发出的询问或保存完成，但不再执行退出。
        if self.closing.confirming == Some(CloseRequest::Quit) {
            self.closing.confirming = Some(CloseRequest::Tabs(Vec::new()));
        }
        self.pending_ui.retain_mut(|action| match action {
            PendingUi::Save(prompt) => {
                if prompt.request == CloseRequest::Quit {
                    prompt.request = CloseRequest::Tabs(Vec::new());
                }
                if prompt.deferred == Some(CloseRequest::Quit) {
                    prompt.deferred = None;
                }
                true
            }
            PendingUi::Close(CloseRequest::Quit) => false,
            _ => true,
        });
        for prompt in [
            &mut self.closing.prompt,
            &mut self.closing.continuation,
            &mut self.format.then_close,
        ]
        .into_iter()
        .flatten()
        {
            if prompt.request == CloseRequest::Quit {
                prompt.request = CloseRequest::Tabs(Vec::new());
            }
            if prompt.deferred == Some(CloseRequest::Quit) {
                prompt.deferred = None;
            }
        }
    }

    /// 保存期间合并关闭请求；退出优先，其次为关闭项目。
    fn defer_close(&mut self, request: CloseRequest) {
        if self.save_as_picker.is_none()
            && !self.format_save_pending()
            && !self
                .pending_ui
                .iter()
                .any(|action| matches!(action, PendingUi::Save(_) | PendingUi::Close(_)))
        {
            return;
        }
        let request = match request {
            CloseRequest::Tabs(ids) => {
                let mut existing = Vec::new();
                for id in ids {
                    if self.editor.index_of(id).is_some() && !existing.contains(&id) {
                        existing.push(id);
                    }
                }
                if existing.is_empty() {
                    return;
                }
                CloseRequest::Tabs(existing)
            }
            request @ (CloseRequest::Quit
            | CloseRequest::CloseProject
            | CloseRequest::CloseFolder
            | CloseRequest::Switch(_)) => request,
            CloseRequest::CloseWindow => return,
        };
        let pending = if let Some(prompt) = self
            .closing
            .continuation
            .as_mut()
            .or(self.format.then_close.as_mut())
            .or(self.closing.prompt.as_mut())
        {
            if prompt.request == request
                || prompt.request == CloseRequest::Quit
                || (prompt.request == CloseRequest::CloseProject
                    && matches!(request, CloseRequest::Tabs(_)))
            {
                return;
            }
            &mut prompt.deferred
        } else if let Some(PendingUi::Save(prompt)) = self
            .pending_ui
            .iter_mut()
            .find(|action| matches!(action, PendingUi::Save(_)))
        {
            if prompt.request == request || prompt.request == CloseRequest::Quit {
                return;
            }
            &mut prompt.deferred
        } else {
            &mut self.deferred_close
        };
        match (pending, request) {
            (slot @ None, request) => *slot = Some(request),
            (Some(CloseRequest::Quit), _) => {}
            (slot, CloseRequest::Quit) => *slot = Some(CloseRequest::Quit),
            (Some(CloseRequest::CloseProject), _) => {}
            (slot, CloseRequest::CloseProject) => *slot = Some(CloseRequest::CloseProject),
            (Some(CloseRequest::CloseFolder | CloseRequest::Switch(_)), CloseRequest::Tabs(_)) => {}
            (slot, request @ (CloseRequest::CloseFolder | CloseRequest::Switch(_))) => {
                *slot = Some(request);
            }
            (Some(CloseRequest::Tabs(existing)), CloseRequest::Tabs(ids)) => {
                existing.retain(|id| self.editor.index_of(*id).is_some());
                for id in ids {
                    if self.editor.index_of(id).is_some() && !existing.contains(&id) {
                        existing.push(id);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    /// 保存完成后重新检查文档和终端，再继续被推迟的关闭。
    pub(in crate::app) fn resume_deferred_close(&mut self, request: CloseRequest) {
        if self.needs_prompt_window() {
            self.pending_ui.push_back(PendingUi::Close(request));
            return;
        }
        match request {
            CloseRequest::Quit => self.request_quit(),
            CloseRequest::CloseProject => self.request_close_project(),
            CloseRequest::Tabs(ids) => self.request_close(CloseRequest::Tabs(ids)),
            CloseRequest::CloseFolder => self.close_folder(),
            CloseRequest::Switch(folder) => self.switch_folder(&folder),
            CloseRequest::CloseWindow => self.request_close_window(),
        }
        if !self.closing_pending()
            && let Some(request) = self.deferred_close.take()
        {
            self.resume_deferred_close(request);
        }
    }

    /// 关闭目录（项目名菜单中的「关闭目录」）：先记录它的现场，再关闭所有标签和终端，回到开始页。
    pub(in crate::app) fn close_folder(&mut self) {
        if self.editor.workspace.is_none() {
            return;
        }
        if self.closing_pending() {
            self.defer_close(CloseRequest::CloseFolder);
            return;
        }
        self.remember_session();
        if !self.confirm_terminal_quit(CloseRequest::CloseFolder) {
            self.request_close(CloseRequest::CloseFolder);
        }
    }

    /// 处理关闭请求 `request`：没有修改的标签直接关闭，有未保存修改的先询问。
    ///
    /// 退出时先备份未保存的内容（热退出），已备份的文档不再询问。
    pub(in crate::app) fn request_close(&mut self, request: CloseRequest) {
        if request == CloseRequest::CloseWindow {
            self.close_window_now();
            return;
        }
        // 正在询问、另存为或保存时，等待其完成，不重复处理。
        if self.closing_pending() {
            self.defer_close(request);
            return;
        }
        // 后台在存的大文件先存完：要不要问、备份哪些，都按存完以后的样子算。
        self.wait_for_saves();
        // 已经在询问时（例如提示框打开期间再次按 `⌘Q`）不再询问第二次。
        if self.closing.prompt.is_some() {
            return;
        }

        // 退出、关闭开着目录的项目时先写入备份，已备份的文档不再询问（热退出），下次启动或再次
        // 打开该目录时恢复。没开目录的项目关闭后，这些备份没有恢复的时机，因此逐个询问。
        let backed_up = match request {
            CloseRequest::Quit => self.back_up_for_quit(),
            CloseRequest::CloseProject if self.editor.workspace.is_some() => {
                self.back_up_for_quit()
            }
            _ => Default::default(),
        };
        let dirty: Vec<DocumentId> = self
            .editor
            .documents
            .iter()
            .filter(|d| d.is_dirty() && !backed_up.contains(&d.id))
            .filter(|d| match &request {
                CloseRequest::Tabs(ids) => ids.contains(&d.id),
                CloseRequest::Quit
                | CloseRequest::CloseWindow
                | CloseRequest::CloseProject
                | CloseRequest::Switch(_)
                | CloseRequest::CloseFolder => true,
            })
            .map(|d| d.id)
            .collect();

        // 没有修改的标签先关闭，只有剩下的才需要询问。
        let request = match request {
            CloseRequest::Tabs(ids) => {
                let clean: Vec<_> = ids
                    .iter()
                    .copied()
                    .filter(|id| !dirty.contains(id))
                    .collect();
                self.remove_tabs(&clean);
                CloseRequest::Tabs(dirty.clone())
            }
            other => other,
        };

        if dirty.is_empty() {
            self.finish_close(request);
            return;
        }

        if self.needs_prompt_window() {
            self.pending_ui.push_back(PendingUi::Close(request));
            return;
        }
        self.closing.prompt = Some(SavePrompt {
            request,
            dirty,
            deferred: None,
        });
        self.show_save_prompt();
    }

    /// 弹出保存提示框。
    ///
    /// 没有窗口（测试中）时只保留 `Closing::prompt`，等待测试代码直接回答。
    fn show_save_prompt(&mut self) {
        let Some(prompt) = &self.closing.prompt else {
            return;
        };
        let names: Vec<String> = prompt
            .dirty
            .iter()
            .filter_map(|&id| self.editor.document(id))
            .map(|d| d.name().into_owned())
            .collect();

        let (message, detail, save) = if let [name] = names.as_slice() {
            (
                t!("workspace-close-save-one", name = name.as_str()),
                t!("workspace-close-save-detail").to_owned(),
                t!("workspace-close-save"),
            )
        } else {
            // 文件过多时只列出前 `SHOWN` 个，避免提示框高度超过屏幕。`SHOWN` 的单位为个，是经验值。
            const SHOWN: usize = 10;
            let mut list = names.iter().take(SHOWN).cloned().collect::<Vec<_>>();
            if names.len() > SHOWN {
                list.push(t!(
                    "workspace-close-more-files",
                    count = names.len() - SHOWN
                ));
            }
            (
                t!("workspace-close-save-many", count = names.len()),
                format!(
                    "{}\n\n{}",
                    list.join("\n"),
                    t!("workspace-close-save-detail")
                ),
                t!("workspace-close-save-all"),
            )
        };

        // 没有窗口（测试中）时不弹出，由测试直接回答（`answer_save_prompt`）。
        if self.window.handle.is_none() {
            return;
        }
        self.show_window();
        let buttons = vec![
            AlertButton::new(save),
            AlertButton::cancel(t!("common-cancel")),
            AlertButton {
                key: Some(AlertKey::Command('d')),
                ..AlertButton::new(t!("workspace-close-dont-save"))
            },
        ];
        let shown = self.show_alert(&message, &detail, buttons, None, move |index, _| {
            Some(UserEvent::SaveChoice(SaveChoice::from_index(index)))
        });
        // 窗口上已有其他面板，提示框没有弹出：放弃这次询问，不关闭任何内容。
        if !shown {
            self.closing.prompt = None;
        }
    }

    /// 处理提示框上的选择 `choice`：取消则不做任何事，不保存则直接关闭，保存则先保存再关闭。
    pub(in crate::app) fn answer_save_prompt(&mut self, choice: SaveChoice) {
        let Some(prompt) = self.closing.prompt.take() else {
            return;
        };

        match choice {
            SaveChoice::Cancel => self.deferred_close = None,
            SaveChoice::Discard => {
                let mut prompt = prompt;
                prompt.dirty.clear();
                self.save_then_close(prompt);
            }
            SaveChoice::Save => self.save_then_close(prompt),
        }

        self.request_redraw();
    }

    /// 按顺序保存 `prompt.dirty` 中的文档，全部保存后再执行关闭。
    ///
    /// 遇到未命名的文档时弹出「另存为」面板，选好路径后由 `on_picked` 回到这里；开启了保存时
    /// 格式化的文档先格式化，格式化并保存完成后由 `save_with_note` 回到这里。保存失败时中止，
    /// 不关闭。
    pub(in crate::app) fn save_then_close(&mut self, mut prompt: SavePrompt) {
        if self.needs_prompt_window() {
            self.pending_ui.push_back(PendingUi::Save(prompt));
            return;
        }
        if self.format_save_pending() || self.save_as_picker.is_some() {
            self.format.then_close = Some(prompt);
            return;
        }
        while let Some(&id) = prompt.dirty.first() {
            let Some(doc) = self.editor.document(id).filter(|d| d.is_dirty()) else {
                prompt.dirty.remove(0);
                continue;
            };

            if doc.path.is_none() {
                self.closing.continuation = Some(prompt);
                self.pick(Pick::SaveAs(id));
                return;
            }
            if self.prepares_on_save(id) {
                self.format.then_close = Some(prompt);
                self.format_then_save(id);
                return;
            }
            if !self.save_document(id, None) {
                return;
            }
            prompt.dirty.remove(0);
        }

        let deferred = prompt.deferred.or_else(|| self.deferred_close.take());
        self.finish_close(prompt.request);
        if let Some(request) = deferred {
            self.resume_deferred_close(request);
        }
    }

    /// 在所有未保存的修改都已保存或放弃之后，执行 `request` 对应的关闭。
    fn finish_close(&mut self, request: CloseRequest) {
        match request {
            CloseRequest::Tabs(ids) => self.remove_tabs(&ids),
            // 退出时不关闭标签：退出前还需要把标签记入现场。
            CloseRequest::Quit => {
                self.quit_backups();
                self.closing.quit_requested = true;
            }
            CloseRequest::CloseWindow => self.close_window_now(),
            CloseRequest::CloseProject => self.close_project_now(),
            CloseRequest::Switch(folder) => {
                self.remove_all_tabs();
                self.close_all_terminal_tabs();
                self.open_folder_now(&folder);
            }
            CloseRequest::CloseFolder => {
                self.remove_all_tabs();
                self.close_all_terminal_tabs();
                self.set_terminal_open(false);
                self.close_folder_now();
            }
        }
        self.relayout();
        self.request_redraw();
    }

    /// 关闭窗口而不退出应用（macOS）。
    ///
    /// 窗口里放着所有项目，项目和运行中的终端都留着；记下备份、窗口大小和目录现场后隐藏。
    fn close_window_now(&mut self) {
        self.vim_persist();
        self.remember_window();
        if self.editor.workspace.is_some() {
            self.back_up_for_quit();
        }
        self.remember_session();
        self.hide_window();
    }

    /// 关闭这个项目：标签、目录和终端都结束，窗口交给相邻的项目。
    ///
    /// 与退出时一样先记下目录的现场、把未保存的内容写进备份；这是最后一个项目时留在开始页上，
    /// 不置 `project_closed`。
    fn close_project_now(&mut self) {
        self.remember_window();
        self.quit_backups();
        self.remember_session();
        self.leave_backups_on_disk();
        self.remove_all_tabs();
        self.close_all_terminal_tabs();
        self.set_terminal_open(false);
        if self.editor.workspace.is_some() {
            self.close_folder_now();
        }
        self.close_settings();
        self.project_closed = self.shared.others();
        self.focus = Focus::Editor;
        self.relayout();
        self.request_redraw();
    }

    fn remove_all_tabs(&mut self) {
        let ids: Vec<DocumentId> = self.editor.documents.iter().map(|d| d.id).collect();
        self.remove_tabs(&ids);
    }

    /// 直接关闭 `ids` 对应的标签，不询问。
    ///
    /// 全部关完以后才重新排版一次：逐个排版时，中途轮到当前的每个文档都会按自动换行整篇折一遍，
    /// 下一步又被关掉。
    pub(in crate::app) fn remove_tabs(&mut self, ids: &[DocumentId]) {
        let mut closed = false;
        for id in ids {
            if let Some(index) = self.editor.index_of(*id) {
                self.remove_tab(index);
                closed = true;
            }
        }
        if closed {
            self.relayout();
            self.request_redraw();
        }
    }

    /// 直接关闭标签 `id`，不询问、不播放关闭动画、不记入最近关闭的标签；用于替换预览标签。
    pub(in crate::app) fn discard_tab(&mut self, id: DocumentId) {
        let Some(index) = self.editor.index_of(id) else {
            return;
        };
        self.cancel_format(id);
        if self.vim.is_some() {
            self.vim_store_marks(id);
        }
        if let Some(doc) = self.editor.close(index) {
            self.forget_git_document(doc.id, doc.path.as_deref());
        }
        self.search.current = None;
        self.check_waiting();
    }

    /// 直接关闭第 `index` 个标签，不询问，也不重新排版（由 [`App::remove_tabs`] 统一排版）。
    fn remove_tab(&mut self, index: usize) {
        if let Some(doc) = self.editor.documents.get(index) {
            let id = doc.id;
            self.remember_closed(doc.path.clone(), doc.selection.head);
            self.cancel_format(id);
            if self.vim.is_some() {
                self.vim_store_marks(id);
            }
        }
        self.animate_tab_close(index);
        if let Some(doc) = self.editor.close(index) {
            self.forget_git_document(doc.id, doc.path.as_deref());
        }
        self.search.current = None;
        self.check_waiting();

        if self.editor.documents.is_empty() {
            // 最后一个标签关闭后，查找条和跳转条随之关闭。「只显示匹配的行」也必须关闭，
            // 否则之后打开其他文件并查找下一个时，该文件会因残留的筛选状态而被隐藏一部分行。
            self.find.open = false;
            self.find.filter_lines = false;
            let goto_open = self.goto_input.take().is_some();

            // 文件树中输入名字同样使用 `Focus::Prompt`；只有跳转条打开时，该焦点才属于跳转条。
            if matches!(self.focus, Focus::Search | Focus::Replace)
                || (goto_open && self.focus == Focus::Prompt)
            {
                self.focus = Focus::Editor;
            }
        }
    }

    /// 记录已关闭的标签（路径和光标位置，`caret` 为字符偏移）。
    ///
    /// 未命名的文档无法重新打开，不记录；同一个文件只保留最近一次。
    fn remember_closed(&mut self, path: Option<PathBuf>, caret: usize) {
        // 最多保留的已关闭标签数（个，经验值），超出时丢弃最早的。
        const MAX_CLOSED: usize = 20;

        let Some(path) = path else {
            return;
        };
        self.closed.retain(|(p, _)| *p != path);
        self.closed.push((path, caret));
        if self.closed.len() > MAX_CLOSED {
            self.closed.remove(0);
        }
    }

    /// 重新打开最近关闭的标签（`⇧⌘T`），光标回到关闭时的位置。
    ///
    /// 文件已不存在时跳过该项，继续向更早的记录查找；文件已经打开时直接切换到该标签。
    /// 没有可重新打开的标签时在状态栏提示。
    pub(in crate::app) fn reopen_closed(&mut self) {
        while let Some((path, caret)) = self.closed.pop() {
            if !path.is_file() {
                continue;
            }

            // 读好以后再恢复光标位置。
            self.open_path_then(&path.clone(), move |app| {
                if let Some(doc) = app.editor.active_mut()
                    && doc.path.as_deref() == Some(path.as_path())
                {
                    let end = doc.text().end_pos();
                    doc.selection = Selection::point(caret.min(end));
                }
                app.ensure_caret_visible();
            });
            self.request_redraw();
            return;
        }

        self.flash(t!("workspace-reopen-tab-none"));
    }
}

/// 关闭与退出流程的状态：进行中的保存确认、终端确认，以及是否已可以退出。
#[derive(Default)]
pub struct Closing {
    /// 正在询问「要保存修改吗」。
    pub(crate) prompt: Option<SavePrompt>,
    /// 已选择「保存」、正在等待另存为面板：保存完成后继续处理余下的步骤。
    pub(crate) continuation: Option<SavePrompt>,
    /// 正在确认是否结束终端中运行的程序时，确认后要执行的关闭（退出或关闭窗口）。
    ///
    /// 为 `Some` 期间，退出流程尚未得出结果（见 [`crate::winc`]）。
    pub(crate) confirming: Option<CloseRequest>,
    /// 退出前的处理均已完成；当前一轮事件处理结束后退出。
    pub(crate) quit_requested: bool,
}

#[cfg(test)]
mod tests;

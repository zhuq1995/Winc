# Git 暂存管理：现有实现

## 提交模型

- `crates/winc-app/src/app/git/commit.rs:140` 的 `CommitGroup` 将条目分为冲突、更改、未跟踪；已暂存的新文件与已跟踪文件共同显示在更改组。
- `crates/winc-app/src/app/git/commit/state.rs:55` 的 `checked` 根据内存中的 `included` / `excluded` 记录决定提交选择。
- `crates/winc-app/src/app/git/commit/state.rs:65` 的 `file_state` 明确以工作区内容为提交来源；终端中的暂存状态与该选择相互独立。
- `crates/winc-app/src/app/git/commit/state.rs:84` 的 `hunk_inclusions`、`toggle_hunk` 和 `toggle_line` 提供文件内的改动块与单行选择。
- `crates/winc-app/src/app/git/commit/actions.rs:163` 的 `git_commit` 保存勾选文件的编辑器修改，检查冲突，再准备完整文件与部分改动的提交输入。
- `crates/winc-editor/src/git/commit.rs:41` 的 `Git::commit` 接收文件列表、未跟踪文件与部分内容。普通整文件提交使用 `git commit --only`，部分提交在临时索引内构造目标版本；合并、拣选和还原操作使用独立分支处理。

## Git 状态与差异

- `crates/winc-editor/src/git.rs:301` 的 `Entry` 已分别持有 `staged`、`unstaged` 和改名前路径 `from`；同一文件能同时具有两侧变化。
- `crates/winc-editor/src/git.rs:1106` 的 `content_at` 支持读取暂存区内容，空版本字符串代表暂存区。
- `crates/winc-app/src/app/git.rs:329` 的 `prepare` 当前比较 HEAD 与工作区；编辑器打开文件时使用包括未保存修改的内存内容。
- `crates/winc-app/src/app/git.rs:1113` 的 `fetch_changes` 在后台准备差异；`poll_git_content` 按仓库与预览序号丢弃过期结果。
- 真实暂存模型要求列表条目与差异请求区分暂存侧和工作区侧；同一路径的两类差异具有不同的比较基准。
- `crates/winc-core/src/diff.rs:437` 的 `LineSet`、`Pick` 与 `pick_changes` 已支持改动块和单行的内容组合，可以复用文本选择算法。
- `crates/winc-editor/src/git.rs:1106` 的 `content_at` 读取的是经过检出过滤器的内容。部分暂存的写入必须区分展示文本与 Git blob 字节；CRLF、BOM、过滤器及缺失文件状态需要独立覆盖。
- `crates/winc-editor/src/git.rs:1316` 的 `relative_all` 使用 `:(literal)` 路径参数，暂存文件名中的 `[`、`*`、`?` 时沿用该契约。
- `crates/winc-app/src/app/git/branch.rs:667` 的 `git_operate` 对分支写操作提供忙碌标志；提交另有 `commit.busy`，暂存需要与这两类操作共同互斥。
- `crates/winc-app/src/app/git.rs:739` 的状态刷新在操作完成后可复用；差异刷新当前主要依赖 HEAD 与文档变化，新增暂存侧差异需要覆盖仅 index 变化的刷新。

## 交互与测试入口

- `crates/winc-app/src/app/git/commit.rs:1541` 的 `run_commit_file_command` 支持多选文件的勾选、取消勾选、回滚和复制路径。
- `crates/winc-app/src/app/snapshot/git/commit_selection.rs:11` 覆盖 Ctrl/Cmd 单击、Shift 选择、多文件批量动作。
- `crates/winc-app/src/app/snapshot/git/commit.rs:43` 覆盖只提交选定改动块，`:2320` 覆盖只提交选定行。
- `crates/winc-editor/src/git/tests.rs:630` 覆盖部分提交保留其他已暂存文件与执行位。
- `.github/CONTRIBUTING.md` 要求行为测试、视觉快照、双语文案，并保持修改集中；新增操作复用现有 Git 信任、命令执行、超时和错误展示机制。
- `.trellis/spec/backend/window-projects.md` 规定后台操作结果归属发起项目，切换项目保留 Git 状态。
- `crates/winc-app/src/app/workspace/files.rs:572` 的 `write_document` 是同步保存入口，等待同文档在途的后台保存，并检查磁盘冲突；`:585` 的 `write_document_with` 区分同步保存与后台写入。
- `crates/winc-app/src/app/editor/format.rs:814` 的 `format_save_pending` 表示保存前格式化或工具确认仍未完成。暂存需等待相关保存实际完成，取消／失败时终止操作；块与单行选择在格式化改变内容后重新核对。

## 规划决策

- 用户选择真实 Git 暂存区作为唯一提交来源，提交面板区分已暂存与未暂存变化。
- 用户选择首期支持文件、批量、改动块和单行的暂存与取消暂存。
- 用户选择暂存区为空时，经确认暂存并提交全部更改（包含未跟踪文件）。
- 用户选择暂存前保存相关文件；取消暂存保留工作区内容，提交只读取暂存版本。

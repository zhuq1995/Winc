# Git 暂存管理

## 目标

在 Winc 的 Git 面板中提供类似 VS Code 的暂存体验，让用户把不同目的的改动分开提交，并通过界面准确判断下一次提交的内容。

## 背景与现有事实

- 当前提交面板通过内存中的勾选状态选择文件、改动块和单行，提交来源为工作区内容（`crates/winc-app/src/app/git/commit/state.rs:65`）。
- 列表分为冲突、更改、未跟踪三个组；已暂存与未暂存的已跟踪文件混在更改组（`crates/winc-app/src/app/git/commit.rs:140`）。
- 底层状态分别记录已暂存与未暂存变化，同一文件可以同时有两类变化（`crates/winc-editor/src/git.rs:301`）。
- 差异视图当前比较 HEAD 与工作区，打开文件时包含编辑器中的未保存内容（`crates/winc-app/src/app/git.rs:329`）。
- 详细调研与其余代码入口见 `research/current-git-flow.md`。

## 需求与产品决策

### R1：统一提交来源

- 真实 Git 暂存区是普通提交与修改上次提交时的内容来源，Winc 与终端共享该状态。
- 原有内存勾选提交模型由暂存操作取代，面板按钮、快捷键与“提交并推送”使用相同的提交范围。
- 暂存后继续编辑文件时，后续改动留在工作区，提交读取暂存版本；仅修改上次提交信息的入口保持可用。

### R2：分组与差异

- 面板按“合并更改”（有冲突时）、“已暂存的更改”、“更改”展示；各组支持现有树形／平铺、折叠与滚动方式。
- 未跟踪且未被忽略的文件列在“更改”中，保持新文件标识。
- 同一文件同时有两类变化时分别出现在“已暂存的更改”和“更改”中，选择、上下文菜单与差异跟随所在组。
- 已暂存条目比较 HEAD 与暂存版本，内容只读；更改条目比较暂存版本与工作区，继续支持编辑器未保存内容的展示并标明未保存状态。
- 文件行和组标题分别提供取消暂存（−）与暂存（＋）入口，文件右键菜单、多选和命令入口提供对应操作。

### R3：项目与平台兼容

- 暂存操作作用于当前所选仓库／工作树；项目切换后后台结果仍归发起项目，关闭项目后的迟到结果沿用现有丢弃规则。
- 状态更新覆盖终端中的 `git add`、`git reset`、提交及其他仅改变暂存区的操作。
- 简体中文与英文提供一致文案；Windows、macOS、Linux 与现有 WSL Git 路径处理保持兼容。
- 保留提交信息草稿、提交并推送、修改上次提交、撤销上次提交、合并与变基入口；Git 操作继续使用现有信任、错误展示和确认规则。

### R4：操作粒度

- 首期支持单文件、多文件、组内全部、目录内文件，以及文本差异中的改动块和单行暂存／取消暂存。
- 块与单行操作支持现有可处理的 UTF-8 普通文本；新增、修改和删除文件的文本内容均有明确的部分操作行为。
- 二进制、大文件、符号链接、子模块及无法安全映射到 Git 内容的编码／过滤器状态沿用整文件操作，界面说明部分操作不可用的原因。
- 重命名与执行权限等元数据由整文件操作处理；部分文本操作保留未选中的内容与元数据，并区分文件缺失与空文件。

### R5：暂存区为空时提交

- 普通提交在工作区有可提交改动而暂存区为空时弹出确认，列明将暂存并提交当前仓库的全部更改，包含未跟踪且未被忽略的文件。
- 确认后先完成暂存再提交；取消保持文件、暂存区与提交信息原状。暂存失败时停止提交并显示原因。
- 暂存成功但提交被钩子、身份或签名等原因拒绝时，保留暂存内容和提交信息以便重试。
- 合并、拣选、还原与变基中的继续操作遵循对应 Git 状态，冲突处理要求优先于普通空暂存提示。

### R6：保存与内容保留

- 暂存前保存所操作文件的未保存修改，并等待相关在途保存完成；保存失败、冲突或取消时停止后续暂存。
- 块与单行操作依据保存后的差异。保存、格式化或外部修改导致选择无法准确对应时，刷新差异并提示重新选择。
- 取消暂存只改变暂存区，工作区文件和未保存文档保持原状。
- 用户明确选择的暂存内容在操作完成前发生变化时，提示刷新／重试，禁止静默扩大为整文件操作。

### R7：失败与边界状态

- 处理尚无首个提交、已暂存新增／删除／重命名、同一文件两侧变化、部分暂存、暂存区锁占用及 Git 失败。
- 冲突单独显示；解决内容保存后允许整文件暂存，仍有冲突标记时提示处理，合并中禁止部分暂存未解决的冲突条目。
- 暂存／取消暂存与 Winc 自身的提交、切分支及其他仓库写操作互斥；失败后刷新真实状态，保留提交信息。
- 回滚／丢弃内容沿用独立确认规则，取消暂存的按钮与文案明确表达保留工作区更改。
- 状态列表达到现有数量上限时显示截断提示，全部操作的范围标明当前仓库，确认范围覆盖 Git 实际操作对象。

## 验收标准

- [x] A1（R1、R2）：文件保存为 v2 后暂存，再保存为 v3；文件同时出现在两组，已暂存差异显示 v1→v2，更改差异显示 v2→v3，提交结果为 v2。
- [x] A2（R1、R3）：在终端暂存／取消暂存文件后界面反映真实状态；重启 Winc 后保留 Git 暂存状态。
- [x] A3（R2、R4、R6）：文件、多选、目录和全部操作均更新正确组；取消暂存后的磁盘文件与未保存编辑内容保持原状。
- [x] A4（R4、R6）：一个文本文件包含两处改动，仅暂存其中一块或一行并提交，未选中内容仍留在工作区；部分取消暂存同样只移除选定变化。
- [x] A5（R5）：暂存区为空时，确认框明确包含未跟踪文件；取消保持原状，确认后暂存并提交；提交失败保留暂存内容与信息。
- [x] A6（R6）：暂存未保存文件时先完成相关保存；保存失败、磁盘冲突、格式化或外部改动使选择失效时停止并提示，暂存区保持该步骤前状态。
- [x] A7（R3、R7）：在项目 A 发起暂存后切到 B，结果只更新 A；不同工作树的暂存区独立，关闭项目后的结果保持现有生命周期规则。
- [x] A8（R4、R7）：无首个提交、空文件、新增、删除、重命名、混合两侧变化、CRLF 与带特殊字符路径具有针对性覆盖；受限文件有整文件入口与说明。
- [x] A9（R3、R7）：修改上次提交、撤销提交、提交并推送、冲突解决与变基继续使用一致的暂存语义；锁冲突和操作失败显示原因并允许刷新重试。
- [x] A10（R2、R3）：树形／平铺、折叠、多选、键盘、亮暗主题和窄侧栏布局具有交互或截图覆盖；两种语言和可用平台构建检查记录在任务中。

## 验收记录

- 自动化覆盖（`cargo test -p winc-editor` 388 通过、`cargo test -p winc-app` 817 通过、`cargo test -p winc-i18n` 9 通过；`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings` 通过）：
  - A1 `a_commit_takes_the_index_not_the_worktree`、`a_file_staged_in_the_terminal_appears_in_both_groups`、`staging_refreshes_the_open_diff_baseline`
  - A2 `a_file_staged_in_the_terminal_appears_in_both_groups`（终端 `git restore --staged` 后界面更新）；重启保留由 Git index 本身承担，没有新增状态文件
  - A3 `commit_view_stages_by_file_and_commits_the_index`、`command_and_shift_clicks_pick_several_files_in_one_group`、`commit_list_groups_changes_by_directory`、`the_more_menu_stages_and_unstages_everything`
  - A4 `staging_one_hunk_keeps_the_other_hunk_in_the_working_tree`、`staging_one_line_leaves_the_other_added_line_behind`、`unstaging_one_hunk_keeps_the_other_one_staged`、`partial_staging_writes_only_the_selected_hunks_and_lines`
  - A5 `committing_an_unsaved_document_saves_it_before_staging`、`a_refused_commit_keeps_the_index_and_the_message`、`a_refused_stage_all_keeps_the_working_tree_and_the_message`
  - A6 `staging_a_file_changed_outside_the_editor_stops_before_saving`、`a_stale_selection_is_refused_instead_of_staging_the_file`
  - A7 `poll_stage` 按仓库根丢弃其他仓库的结果；占用按规范化工作树根目录记录（无自动化时序测试）
  - A8 `unstaging_before_the_first_commit_removes_the_entries`、`partial_staging_new_deleted_and_empty_files`、`staging_a_rename_covers_the_old_path_too`、`partial_staging_keeps_crlf_bom_and_missing_final_newline`、`partial_staging_does_not_touch_files_with_similar_names`、`partial_staging_rejects_binary_and_conflicted_content`
  - A9 `amending_an_empty_index_does_not_stage_everything`、`the_last_local_commit_can_be_undone`、`files_with_conflict_markers_are_not_committed`、`committing_while_a_rebase_stopped_continues_it`、`partial_staging_reports_a_locked_index`
  - A10 树形／平铺、折叠、多选、键盘由交互测试覆盖；`shoot_commit_view`、`shoot_commit_widths` 为手动截图测试

## 已知限制

- 截图测试为 `#[ignore]`，本次未重新生成与查看产物；macOS、Linux 与 WSL 的构建和窗口交互未在本机验证。
- A6 中「保存前格式化在途时暂停暂存」只有代码路径与注释，没有能延迟保存的格式化器测试。
- A7 的跨项目迟到结果与同工作树占用没有自动化时序测试：测试中的后台任务同步完成，无法制造结果未到达即切换的时序。
- 状态列表超过 10,000 条时「全部」不再遍历条目（按当前仓库范围交给 Git），该分支没有万级文件的自动化测试。
- 符号链接只能拒绝部分暂存；Windows 上创建符号链接需要权限，无自动化覆盖。
- `cargo test --workspace` 中 `winc-pty --test windows` 的 `a_program_runs_in_a_pseudo_console_and_reports_its_exit_code` 在本机既有失败（未改动基线同样失败），与本任务无关。

## 范围之外

- 并行保留一套基于内存勾选的直接提交流程。
- 新增文件历史、储藏管理、远程协作、冲突合并编辑器或通用 Git 教学页面。
- 增加 Git 状态数量上限、重新设计编辑器差异算法，或引入新的 Git 库。
- 对二进制或无法安全逐行映射的内容实现行级暂存。

## 任务与材料

- 开发分支：`feat/git-staging`；集成目标：`dev`。
- 任务状态：`planning`；实施由最终方案审阅后的明确批准授权。
- 规划文件通过目录联接保存在 `chore/trellis-tooling` worktree，遵守 `AGENTS.md` 的版本管理约定。

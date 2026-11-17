# 实施计划：Git 暂存管理

## 目录按钮修复

- 开发分支：`feat/restore-directory-staging`，集成目标：`dev`；本节限定目录批量暂存入口。
- [x] 目录行新增 `DirStage` 命中，复用 `stage_paths` 与现有暂存队列；按当前组展开折叠子目录。
- [x] 接入普通与顶部固定目录的悬停、图标、提示、长名展开和菜单；历史、勾选式与冲突目录隐藏按钮。
- [x] 跨平台回归模块覆盖真实 Git 索引、鼠标点击、组边界、首次提交前取消暂存和过期命中。
- [x] 运行 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test -p winc-app`，记录执行平台与审阅结果。
- [x] 将目录入口与测试路径补入功能保留清单，提交本次修复并合回 `dev`。
- 检查平台：Windows。格式检查与 workspace Clippy（`-D warnings`）通过；`cargo test -p winc-app` 为 769 通过、88 忽略。目录模块 6 个用例运行真实 Git，包含长名与固定目录被部分推出时的可见范围和点击。
- 本轮包含目录控件、当前组批量范围与保留清单；原任务的其他平台和视觉验收状态沿用下文。

## 实施前门槛

- [ ] 用户在最终规划摘要之后明确批准实施。
- [ ] 当前任务为 `.trellis/tasks/11-10-git-staging`，开发分支为 `feat/git-staging`，集成目标为 `dev`。
- [ ] 读取 `prd.md`、`design.md`、`research/current-git-flow.md` 和清单引用的项目约定。
- [ ] `implement.jsonl` 与 `check.jsonl` 含真实上下文，`task.py validate` 通过。
- [ ] 审阅工作区现有修改，确认 Trellis 文件随项目在当前开发分支管理。
- [ ] 批准后运行 `task.py start`；实施与检查子代理的提示以 `Active task: <task.py current 返回路径>` 开头。

## 实施顺序

1. [x] **真实 index 操作与提交来源**：在 `winc-editor::git` 增加整文件暂存／取消暂存、首次提交前取消暂存和读取 index 内容的接口；提交改为直接提交真实 index，保留 amend 与进行中 Git 操作契约。用临时仓库覆盖 A1、A2、A8、A9。
2. [x] **部分操作**：复用 `winc-core::diff` 的块／行选择，增加最小 Git patch 适配，使用 `git apply --cached` 写入。覆盖暂存与反向取消、缺失／空文件、部分新增／删除、CRLF、BOM、末尾换行和锁冲突；受限编码或过滤器给出整文件入口。检查过期选择后禁止整文件兜底（A4、A6、A8）。
3. [x] **分组和差异身份**：增加 staged／worktree 比较类型，行、选区和差异缓存包含组；同一路径可出现两次。刷新既覆盖 HEAD 变化，也覆盖 index-only 变化，更新两侧基准与只读属性（A1、A2、A3）。
4. [x] **界面与输入**：在现有提交面板接入三个分组、文件／组的＋和−、目录批量动作、同组多选、右键菜单、空格操作、命令面板与块／单行入口。保留现有折叠、树形／平铺、滚动、编辑和回滚确认（A3、A4、A10）。
5. [x] **保存与操作调度**：暂存前完成相关文档保存，处理在途写入、保存失败、格式化确认及选择变化；与提交和分支写操作互斥。后台结果保持项目与仓库归属，关闭／切仓库清理未执行续接（A6、A7、A9）。
6. [x] **空暂存与提交配套**：统一按钮、快捷键及提交并推送，接入空暂存确认和保存→暂存全部→提交顺序，失败时保留 index 与提交信息。回归 amend、撤销提交、合并／拣选／还原和变基继续；状态截断时全部范围为当前仓库（A5、A8、A9）。
7. [ ] **双语、视觉和平台**：两套 Fluent 文案已同步，i18n 键一致与未使用消息测试通过；截图测试已改用新控件但未重新生成产物，Windows 实际窗口交互与 macOS／Linux／WSL 检查未完成（见 `prd.md` 的已知限制）。
8. [x] **全范围检查与规范**：派发 `trellis-check` 检查全部改动和验收映射，处理发现；使用 `trellis-update-spec` 保存适用契约；按明确路径整理当前开发分支的代码与工具文件提交，再进入任务收尾。

## 验证方式

首先运行针对实际改动的 Git 临时仓库和 UI 测试，涉及 core 或语言资源时包含对应包：

```bash
cargo test -p winc-editor
cargo test -p winc-app
cargo test -p winc-core
cargo test -p winc-i18n
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask test --all
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

视觉检查使用仓库现有 `#[ignore]` 截图测试入口，按具体新增测试名称运行并查看产物。若改动影响大列表绘制、缓存或滚动，运行 `cargo xtask bench` 并比较相关结果。

手动场景：

- 同一文件 v1→v2 暂存→保存为 v3，检查两个差异和最终提交。
- 暂存／取消暂存块与单行，检查磁盘、未保存内容与其他已暂存文件。
- 在终端更改 index，观察列表和已打开差异更新。
- 空暂存提交的确认、取消、失败重试，包含未跟踪文件和忽略文件。
- 保存失败、格式化改变选择、index 锁占用、提交钩子拒绝。
- 项目 A 写 Git 时切换到 B，检查结果、通知与仓库归属；不同工作树独立操作。
- 树形／平铺、多选、空格、右键菜单、窄侧栏和两种语言。

## 风险与回退点

- `app/git/commit.rs`、`commit/state.rs` 与 `view/git/commit.rs` 的单路径行身份必须一起更新，避免跨组误操作。
- `app/git.rs` 的差异刷新与 `winc-editor::git::content_at` 的内容转换一同审查，避免显示版本与暂存字节不一致。
- 提交、变基继续与保存链的互斥和错误续接同时检查，禁止保存失败后继续暂存／提交。
- 拆分为可审阅的底层、界面和集成修改；回退仅还原代码提交，保留用户 index 与工作区内容。
- 某平台环境或 CI 无法使用时记录未完成检查与原因，验收项保持未完成；代码测试失败时保留任务进行中。

## 差异基准修复

- [x] 比较类型落实为三份基准：暂存区↔工作区（「更改」组、命令与文件树入口）、上次提交↔暂存区（「已暂存的更改」组，右侧只读）、上次提交↔工作区（勾选式与冲突中的文件）。
- [x] 差异内的块／行操作按当前基准决定方向：暂存区的差异写入暂存区，「已暂存的更改」的差异退回暂存区。
- [x] 行号栏改动标记与回滚改为相对暂存区；暂存区变化（界面写入或其他程序执行 `git add`）时重新计算。
- [x] 差异缓存按暂存区指纹失效；同一路径同时列在两组时，命中带组信息，各自打开对应的基准。
- 检查平台：Windows。`cargo test -p winc-app` 769 通过、88 忽略；`cargo test -p winc-core` 96 通过；`cargo test -p winc-i18n` 9 通过。`snapshot::git` 的用例位于 `#[cfg(unix)]` 模块，本机临时启用后运行得到 227 通过、26 失败，与改动前的 27 个平台失败相比修复 1 个、无新增，运行后已还原模块开关。

## 本地 Windows 打包

- 构建提交 `ef77ec8`（`dev`），版本 `0.1.733`，`msiVersion` `0.1.733`。
- 产物：`target/x86_64-pc-windows-msvc/release/bundle/nsis/Winc-0.1.733-windows-x64-setup.exe`（5749304 字节）、`…/bundle/portable/Winc-0.1.733-windows-x64-portable.zip`。
- 包内 `winc.exe`、`bundle/stage/winc.exe` 与 release 构建的 SHA-256 同为 `5c59c5f3cd8a9f91d6b9f716715cc8cc07b23bed873714ac8b5a2e9daf3f38c4`；`winc-server-Linux-x86_64` 与 `target/remote-server` 同哈希；便携包 `unzip -t` 无错误，安装包版本资源为 `0.1.733`。
- 本机未配置 Authenticode 与更新公钥：安装包未签名，安装后的程序不检查更新。

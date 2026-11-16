# Winc fork 维护规则

## 分支与上游

- `origin` 是自有 fork，`upstream` 是 `DEEIX-AI/Winc`。常规更新取 `upstream/main`；仅按用户明确要求检查或合并开发线。main 已包含 dev 时使用 main。
- `mirror` 只跟踪上游 main，是只读基线；`dev` 为集成线。私有开发在 `feat/*`，完成后 `git merge --no-ff` 回 dev，分支生命周期控制在一周内。
- 禁止对 dev rebase、force push。修改前核对本地领先提交，推送范围按授权确定。
- main 的常规合并在 dev 上进行；开发线先从 dev 建 `merge/upstream-dev` 试验分支，检查通过后 `--no-ff` 回 dev。用户已批准本轮采用试验分支时沿用该方案。
- 合并前应用工作区干净，已有改动先按归属提交；冲突在 dev 或该轮试验分支上解决一次。

## 工具文件版本管理

- `.trellis/`、`.agents/`、`.pi/` 是主工作区的普通目录，由当前开发分支直接跟踪，随项目变更评审和提交。
- 工具配置、技能、规范、任务与 journal 使用显式路径暂存；提交范围与当前任务对应，禁止 `git add -A`。推送遵守当前分支的授权。
- `.trellis/.gitignore` 排除开发者身份、当前任务指针、运行状态与缓存；技能评估报告保存在 `target/skill-evals/`。
- 新机器 clone 或检出项目分支后直接取得工具文件；用 `git ls-files .trellis .agents .pi`、`git status` 与 `git check-ignore` 核对版本管理范围。

## 改动与冲突取舍

- fork 自有功能优先放新文件、新模块；上游文件优先加调用点，保持改动小。禁止全仓格式化、批量改名或移动上游模块。
- 上游高频文件为 `crates/winc-app/src/winc.rs`、`crates/winc-app/src/app.rs`、`crates/winc-app/src/app/lifecycle/`、`crates/winc-app/src/layout.rs`、`crates/winc-app/src/app/input/`、`crates/winc-app/src/view/workspace/`。提交说明列出触碰的上游函数。
- 窗口、事件路由和项目状态变动前读取 `.trellis/spec/backend/window-projects.md`，按当前源码核对其中符号与覆盖范围。功能契约与当前缺口见 [功能清单](fork-features.md)。
- 上游提供同等功能时复用上游实现并删除 fork 副本；默认值、菜单入口、失败处理与平台边界仍逐项验收。

| 领域 | 当前取舍 |
| --- | --- |
| Git 暂存 | 上游 `winc-core/src/diff/staging.rs` 与应用 `app/git/commit/staging.rs`；块/行保留 Include/IncludeLine，文件行保留 StageFile；四处默认开启 |
| `Git::stage` | `crates/winc-editor/src/git.rs` 单一实现，返回 `Result<String, String>` |
| 命中区域与状态分组 | 采用上游分组；项目的 projects_hits/projects_close/projects_keys 与上游每帧 hits 分别核对 |
| 关闭状态 | deferred_close 为推迟请求，closing.confirming 为终端确认；分别保存与消费 |
| 窗口层 | WindowKey/Request/Slot 保留；App.pointer、App.window、App.frame、App.link 按当前结构承载项目交接 |
| 格式化结果 | 保留文档 id 与 job 匹配以及保存审批 save_id，检查三条结果消费路径 |

上游重写窗口生命周期时，Shell 交接需要重新推导；转为在 upstream tag 上维护补丁集或将功能提交上游，停止整仓合并。先把技术方案记录在任务设计中，实施范围遵守用户已作出的决定。

## 验收与提交

- 保持 LF：应用 worktree 的 `core.autocrlf=false`。Git 临时仓库使用 fixture 或进程级配置保持 LF；保留机器全局设置。合并既有 `GIT_CONFIG_COUNT` 注入，避免覆盖调用者配置。
- Git 测试 PATH 包含 `/c/Program Files/Git/cmd`。至少运行 `cargo test -p winc-app`；合并门禁包含 fmt、workspace clippy 和 `cargo test --workspace --no-fail-fast`。
- 项目切换、终端连续输出、退出确认分别检查；编译通过不代替行为结果。Windows 的 musl 检查只覆盖 Unix 分支编译。
- `winc-pty --test windows` 的伪控制台用例有既有环境失败记录；每轮比较同一基线结果，不能按测试名称自动豁免。
- `.github/workflows/ci.yml` 保持删除，本地执行构建检查；Windows 发布工作流保留。
- 逐个核对本轮上游 feat 提交的实现与测试；每个 F 编号填写合并前后证据，受影响的待验证项与回归阻止完成合并。
- 合并提交说明包含目标 SHA、冲突取舍、上游高频函数和功能保留表。试验分支提交后 `--no-ff` 回 dev，按授权推送；成功后删除该轮试验分支。
- 规范变化更新相关 `.trellis/spec/backend/` 和技能 references，在当前开发分支提交；构建与产物步骤见 [Windows 打包](windows-packaging.md)。

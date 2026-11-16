# 设计：合入 upstream/main 并归并 Git 实现

## 1. 合并流程

```
git switch -c merge/upstream-main dev
git merge --no-ff --no-commit upstream/main
# 逐文件解冲突，每解决一个 git add <文件>
git diff --name-only --diff-filter=U   # 必须为空
cargo fmt --all
cargo clippy --workspace --all-targets
PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast
git commit
git switch dev && git merge --no-ff merge/upstream-main
```

`upstream/main` 的 merge base 是 `5b8d3ea`，分叉点在 PR #181。`upstream/dev` 的内容已全在 `main` 中，不单独合并。

## 2. 冲突分类与取舍

49 个冲突文件按性质分四类，每类一条统一规则，避免逐文件临时判断。

| 类别 | 文件 | 取舍 |
| --- | --- | --- |
| 上游重构、fork 有并行改动 | `app.rs`、`winc.rs`、`app/lifecycle/{window,closing,events,placement,startup}.rs`、`app/input/mouse*`、`app/input/context_menu*`、`view.rs`、`view/workspace/panel.rs`、`keymap/action.rs`、`workspace/{files,recent}.rs` | 以上游结构为骨架，只把 fork 的作业号、格式化参数等与窗口无关的改动贴回去；projects 相关一律不贴 |
| Git 实现与界面 | `app/diff.rs`、`app/git.rs`、`app/git/{commit,commit/actions,commit/list,commit/state,branch,log/tree}.rs`、`view/diff.rs`、`view/git/commit.rs`、`winc-editor/src/git.rs`、`git/commit.rs` | 全取上游；fork 的暂存实现删除，见第 3 节 |
| 测试 | `app/snapshot/git*`、`app/snapshot/windows.rs`、`winc-editor/src/git/tests.rs`、`test_util.rs` | 上游版本覆盖；fork 的 projects 与暂存用例删除，随子任务 2 与第 3 节重建 |
| i18n 与 CI | `crates/winc-i18n/locales/*/*.ftl`、`.github/workflows/ci.yml` | 两语言 ftl 取并集（上游新增键 + fork 自有键），`grep` 校验无重复 key；`ci.yml` 保持删除 |

格式化保存编号单独处理：`app/editor/format.rs` 的三处调用点保留 `job` 参数，`app/snapshot/editor/format.rs` 的用例保留。

## 3. Git 暂存归并

上游已经具备完整的三层结构，fork 的分层与它重合，直接换掉实现层。

| 层 | 上游位置 | 职责 |
| --- | --- | --- |
| 纯计算 | `crates/winc-core/src/diff/staging.rs` | `staged_picks(head, index, work, hunks) -> Vec<Pick>`、`stage(index, work, hunk, line) -> String`、`unstage(head, index, hunk, line) -> String`，275 行含单元测试 |
| 应用层 | `crates/winc-app/src/app/git/commit/staging.rs` | `stage_part(&mut self, entry: &Entry, texts: [&str; 3], (hunk, line): (LineHunk, Option<StageLine>), on: bool)`：先更新界面状态，再把 `StageJob::Part` 交给后台按顺序写索引 |
| 写入 | 上游 `StageJob::{Add, Remove, Part}` 队列 | `git` 命令顺序执行，全部写完后重读状态 |

fork 侧删除：

- `crates/winc-editor/src/git/staging.rs`（`stage_partial`）
- `crates/winc-app/src/app/diff.rs` 的 `stage_diff_hunk`、`stage_diff_line`、`stage_pick`、`StagePick`
- `crates/winc-app/src/view/diff.rs` 的 `DiffTarget::StageHunk`、`DiffTarget::StageLine`
- `app/input/mouse/target.rs` 中对应的命中区域判定

迁移映射（fork → 上游）：

| fork | 上游 |
| --- | --- |
| `StagePick { key, old, line: Option<(side, offset)> }` | `(LineHunk, Option<StageLine>)`，`StageLine::Old/New(绝对行号)` |
| `stage_pick` 内读三份文本 | `texts: [&str; 3]` 由调用方传 `head`、`index`、`work` |
| `Git::stage_partial` 写 `git apply --cached` | `push_staging(StageJob::Part { path, hunk, line, on })` |
| 索引状态重读 | `set_index_text` + 后台 `StageJob` 写完统一重读 |

## 4. projects 代码的处置

fork 的 projects 逻辑不在本任务迁回，从以下位置移除，并在 `5b8d3ea`（分叉点）与 `c14f67e`（拖拽排序完成点）保留可取的原文：

```
crates/winc-app/src/view/workspace/projects.rs
crates/winc-app/src/app/snapshot/windows.rs
crates/winc-app/src/app/lifecycle/window.rs
crates/winc-app/src/app/lifecycle/closing.rs
crates/winc-app/src/app/lifecycle/startup.rs
crates/winc-app/src/app/input/mouse{,/click,/drag,/regions}.rs
crates/winc-app/src/app.rs
crates/winc-app/src/winc.rs
crates/winc-app/src/view.rs
crates/winc-app/src/layout.rs
crates/winc-app/src/app/workspace/files.rs
```

移除后 `crates/winc-app/src/app/lifecycle/window.rs` 与上游一致：窗口句柄在 `App.window.handle`，后台事件经 `WindowProxy`／`Envelope` 回到 `Winc`，窗口间操作走 `Request::{NewWindow, Focus}`。

`dev` 在这个提交上有一个中间状态：单窗口多项目不可用。这是父任务选定的两阶段路径，子任务 2 在其上恢复。

## 5. 高风险点

| 风险 | 判断依据 | 处理 |
| --- | --- | --- |
| 上游删掉 `Shell` 交接后，fork 的 `App` 字段与上游 `App` 分组冲突 | `app.rs` 上游 36 次提交，字段按 `tree`／`sidebar`／`picker`／`find`／`closing` 分组 | 采纳上游分组，删除 fork 的 `projects_*` 字段与 `deferred_close` |
| 上游 `Request::Focus(WindowKey)` 与 fork 的「目录已开就切到那个项目」同义 | `upstream/main:crates/winc-app/src/winc.rs` 的 `Request` 定义 | 子任务 2 用 `Focus` 的语义实现项目切换 |
| i18n 并集可能产生重复 key，`ftl` 重复键在运行时报错 | 两侧都改了 `commands.ftl`、`git.ftl`、`menu.ftl`、`workspace.ftl` | 合并后 `rg -o '^[a-z-]+(?= =)' <ftl> \| sort \| uniq -d` 校验 |
| 上游 `winc-server`、`winc-backend` 引入新依赖与新测试 | `Cargo.toml` 新增 `regex-syntax`、`x11-dl`；`winc-server/tests/remote.rs` 1247 行 | 全 workspace 测试，不只测 `winc-app` |
| 行尾设置导致快照测试失败 | 上游 `397bcbd fix: share LF normalization for shell scripts`、`179c889` | 保持 `core.autocrlf false`，测试前确认 `git config core.autocrlf` |

## 6. 回滚

冲突解决期间任一环节无法收敛时，`git merge --abort` 回到 `dev`，不产生半成品提交。`merge/upstream-main` 分支保留，可继续。

## 7. 已决策

D1（2026-11-16 定）：差异块与差异行的暂存控件取上游复选框。

- `DiffTarget::StageHunk`／`StageLine` 删除，改用 `DiffTarget::Include(usize)`／`IncludeLine(usize, usize)`。
- `app/diff.rs` 的 `stage_diff_hunk`／`stage_diff_line` 删除，走上游 `toggle_diff_hunk`／`toggle_diff_line`。
- `view/diff.rs`、`input/mouse/target.rs` 直接采用上游版本，`icons::line_checkbox`／`line_checkbox_mixed` 原样保留。
- fork 的 ＋／− 只在文件行级保留，因为上游的文件行级本来就是 ＋／−（`CommitTarget::StageFile` + `commit_target_icon`）。

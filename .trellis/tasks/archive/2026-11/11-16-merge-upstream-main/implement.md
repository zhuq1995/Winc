# 执行计划：合入 upstream/main 并归并 Git 实现

冲突由一次 `git merge` 全部产生，下面的批次是解决冲突的推进顺序，不是分开执行的多次合并。每批结束跑该批的验证命令，编译错误收敛在本批范围内。

## B0 准备

- [ ] `git status` 干净；`git config core.autocrlf` 为 `false`。
- [ ] `git switch -c merge/upstream-main dev`。
- [ ] 记录 `git rev-parse HEAD` 为 `<old-dev>`，写入提交说明用。
- [ ] `git merge --no-ff --no-commit upstream/main`。

验证：`git diff --name-only --diff-filter=U | wc -l` 为 49。

## B1 非代码冲突

- [ ] `.github/workflows/ci.yml`：`git rm -f .github/workflows/ci.yml`，保持删除。
- [ ] `git diff --name-only --diff-filter=U | wc -l` 减 1。

## B2 依赖与核心库

- [ ] `Cargo.toml`、`Cargo.lock`：取上游（新增 `winc-backend`、`winc-server`、`regex-syntax`、`x11-dl`）。
- [ ] `crates/winc-core/src/diff/staging.rs`、`crates/winc-core/src/diff.rs`：取上游。
- [ ] `crates/winc-editor/src/git.rs`、`git/commit.rs`：取上游。`Git::stage` 签名两侧一致，无适配。
- [ ] `crates/winc-editor/src/test_util.rs`：取上游，fork 的 `179c889 test: isolate git fixtures from host line-ending settings` 若上游无等价实现则贴回。

验证：`cargo build -p winc-core -p winc-editor`。

## B3 Git 暂存归并

- [ ] 删除 `crates/winc-editor/src/git/staging.rs`，从 `crates/winc-editor/src/git.rs` 移除 `mod staging;` 与 `pub use`。
- [ ] `crates/winc-app/src/app/git/commit/staging.rs`：取上游，确认含 `stage_part` 与 `StageJob`。
- [ ] `crates/winc-app/src/app/diff.rs`：取上游；删除 `stage_diff_hunk`、`stage_diff_line`、`stage_pick`、`StagePick`、`diff_stage_tip`。
- [ ] `crates/winc-app/src/view/diff.rs`：取上游，`DiffTarget::Include`／`IncludeLine` 与 `icons::line_checkbox` 原样保留。
- [ ] `crates/winc-app/src/app/input/mouse/target.rs`、`regions.rs`：整段取上游，`DiffTarget::StageHunk`／`StageLine` 的命中区域随之消失。
- [ ] `crates/winc-app/src/app/git.rs`、`git/{commit,commit/actions,commit/list,commit/state,branch,log/tree}.rs`：取上游。

验证：

```
rg "stage_partial|StagePick|StageHunk|StageLine|stage_diff_hunk|stage_diff_line" crates/   # 期望无输出
rg "toggle_diff_hunk|DiffTarget::Include" crates/winc-app/src   # 期望有命中
cargo build -p winc-app
```

## B4 应用状态与生命周期

- [ ] `crates/winc-app/src/app.rs`：以上游字段分组为骨架，删除 `projects_*`、`deferred_close`、`Project` 相关字段。
- [ ] `crates/winc-app/src/winc.rs`：取上游（`WindowKey`、`Recipient`、`Envelope`、`WindowProxy`、`Request`）；`Project` 结构体随 references 一并删除。
- [ ] `crates/winc-app/src/app/lifecycle/window.rs`：取上游整文件。
- [ ] `crates/winc-app/src/app/lifecycle/{closing,events,placement,startup}.rs`：取上游，删除 `request_close_project`、`can_handoff_for_prompt`、projects 分支。
- [ ] `crates/winc-app/src/app/editor/format.rs`：上游版本上贴回作业号参数（`Running.job`、`Format::next_id`、`poll_formatting`、`formatting_done`、`wait_formatting` 三处一致）。

验证：`cargo build -p winc-app`；`rg "projects_width|can_handoff_for_prompt|take_shell_state|deferred_close" crates/` 无命中。

## B5 输入路由

- [ ] `crates/winc-app/src/app/input/mouse.rs`、`mouse/{click,drag,target}.rs`：取上游，删除 projects 命中与拖拽排序。
- [ ] `crates/winc-app/src/app/input/context_menu{,/menus,/present}.rs`：取上游。
- [ ] `crates/winc-app/src/keymap/action.rs`：取上游；fork 的项目切换命令若被上游新增命令占用同名则保留上游。

验证：`cargo build -p winc-app`。

## B6 视图层

- [ ] `crates/winc-app/src/view.rs`：取上游，移除 `projects` 模块声明与布局调用。
- [ ] `crates/winc-app/src/layout.rs`：取上游；`Layout::projects` 字段删除，traffic light 偏移回上游实现。
- [ ] `crates/winc-app/src/view/workspace/panel.rs`：取上游。
- [ ] 删除 `crates/winc-app/src/view/workspace/projects.rs` 及其 `mod` 声明。
- [ ] `crates/winc-app/src/view/git/commit.rs`：取上游。
- [ ] `crates/winc-app/src/app/workspace/{files,recent}.rs`：取上游；`4481c6a feat: keep the differing part of the path for same-named recent folders` 与 fork 的 `recent.rs` 改动重合，以上游为准。

验证：`cargo build -p winc-app`。

## B7 i18n

- [ ] `crates/winc-i18n/locales/{en-US,zh-CN}/{commands,git,menu,workspace}.ftl`：取并集，上游新键 + fork 自有键。
- [ ] 删除 projects 与暂存按钮专属的键（`workspace-tip-stage-change`、`workspace-tip-stage-line` 等，随 D1 决定）。

验证：

```
for f in crates/winc-i18n/locales/*/*.ftl; do
  rg -o '^[a-z0-9-]+(?= =)' "$f" | sort | uniq -d | sed "s|^|$f: |"
done
```

无输出即无重复 key。

## B8 测试与快照

- [ ] `crates/winc-app/src/app/snapshot/git{,/commit,/commit_selection,/diff,/log,/merge,/rebase}.rs`：取上游。
- [ ] `crates/winc-app/src/app/snapshot/windows.rs`：取上游（projects 用例随子任务 2 重建）。
- [ ] `crates/winc-app/src/app/lifecycle/closing/tests.rs`、`window/tests.rs`：取上游。
- [ ] `crates/winc-editor/src/git/tests.rs`：取上游。
- [ ] fork 的 CI 相关提交 `a2c0be1` 不再需要处理（文件已删除）。

验证：`cargo test -p winc-editor -p winc-app --no-fail-fast`。

## B9 门禁

- [ ] `cargo fmt --all`。
- [ ] `cargo clippy --workspace --all-targets`，无警告。
- [ ] `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast`，除 `winc-pty --test windows` 的伪控制台用例外全绿。

失败处理：先修编译与 clippy，再修测试；测试失败若来自上游新功能（`winc-server` 远程用例），单独确认是否为环境依赖，记录到父任务。

## B10 特性核对与收尾

- [ ] `git log --oneline <old-dev>..upstream/main` 中每个 `feat:` 提交在 `git log --oneline dev..HEAD` 或合并结果里有对应实现：

```
git log --oneline <old-dev>..upstream/main --no-merges | rg '^[0-9a-f]+ feat:' | wc -l
```

按主题抽查：远程开发、预览（markdown/pdf/epub）、Git log 重构、X11 拖放、字体回退、WXML 语法、终端最大化。

- [ ] `git diff --name-only --diff-filter=U` 为空。
- [ ] 提交，说明含：合并来源（`upstream/main` @ `e7d6fef`，VERSION 0.1.733）、冲突取舍（四类规则）、触碰的高频文件。
- [ ] `git switch dev && git merge --no-ff merge/upstream-main`，`git push origin dev`，`git branch -d merge/upstream-main`。

## 回滚点

| 位置 | 回滚方式 |
| --- | --- |
| B1–B8 任一环节 | `git merge --abort` |
| B9 门禁失败 | 留在 `merge/upstream-main` 继续修复，不提交半成品 |
| 合回 `dev` 之后发现回归 | `git revert -m 1 <merge-commit>` |

# Implement — 合入 upstream/dev

## 前置

- [x] `git fetch upstream --prune`；确认 `upstream/main` 0 新增、`upstream/dev` 49 新增。
- [x] 工作区干净（`git status --short` 无输出）。

## 执行清单

1. **建试验分支**
   `git switch -c merge/upstream-dev dev`
2. **合并（不提交）**
   `git merge --no-ff --no-commit upstream/dev`
3. **解 3 处冲突**
   - `git rm .github/workflows/ci.yml`
   - `crates/winc-app/src/app/diff.rs`：fork 暂存逻辑优先，采纳上游新增分支
   - `crates/winc-app/src/view/diff.rs`：上游折叠／滚动条 + fork ＋／− 暂存控件并存
   每解决一个 `git add <文件>`，直到 `git diff --name-only --diff-filter=U` 为空。
4. **核对自动合并的语义点**（见 `design.md`「自动合并但要人工核对的点」）
   `grep -n "StageHunk\|StageLine\|projects_hits\|deferred_close\|formatting_done\|winc-vim" crates -r` 逐条确认。
5. **格式化**
   `cargo fmt --all`
6. **静态检查**
   `cargo clippy --workspace --all-targets`（要求 0 警告）
7. **测试**
   `PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast`
   基线已知失败：`winc-pty --test windows` 伪控制台用例。
8. **提交合并**
   提交说明列出冲突取舍、触碰的上游高频文件、测试结果。
   `git switch dev && git merge --no-ff merge/upstream-dev`
9. **推送与清理**
   `git push origin dev`；`git branch -d merge/upstream-dev`

## 验证命令（复核用）

```bash
git merge-base --is-ancestor upstream/dev dev && echo "upstream/dev 已全部合入"
git log --oneline --grep='^feat' dev ^upstream/dev | wc -l   # 期望 0
cargo fmt --all --check
cargo clippy --workspace --all-targets
PATH="/c/Program Files/Git/cmd:$PATH" cargo test --workspace --no-fail-fast
```

## 记录

- 上游 `feat:` 提交在合并结果里有对应实现与测试：逐个对照 `git log --oneline dev..upstream/dev --grep='^feat'`（22 个）。
- 规范层面的结论（新增取舍、新模块约定）写进 `.trellis/spec/backend/*`，在 `../Winc-trellis` 提交推送。

## 回滚点

| 阶段 | 回滚 |
| --- | --- |
| 合并未提交 | `git merge --abort` |
| 试验分支已提交、验证失败 | `git switch dev && git branch -D merge/upstream-dev` |
| 已合回 `dev`、未推送 | `git reset --hard origin/dev` |

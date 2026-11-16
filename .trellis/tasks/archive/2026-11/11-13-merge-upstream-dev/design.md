# Design — 合入 upstream/dev

## 边界

| 项 | 范围 |
| --- | --- |
| 合入对象 | `upstream/dev` = `5b8d3ea`，基线 `dev` = `9007ce2` |
| 合并基点 | `d79538b`（旧 `upstream/dev` 尖端，已是 `dev` 的祖先） |
| 不在范围 | `upstream/main`（已全含在 `dev`，0 新增）；`upstream/cla-signatures`（签名分支，与代码无关） |
| 产出 | `dev` 上的一个 `--no-ff` 合并提交 + 推送 `origin/dev` |

## 冲突面（`git merge-tree --write-tree --name-only dev upstream/dev`）

只有 3 处冲突，其余 43 个双方都改过的文件可自动合并：

| 文件 | 类型 | 取值 |
| --- | --- | --- |
| `.github/workflows/ci.yml` | modify/delete | 保持删除（fork 不跑上游 CI）。冲突解决时 `git rm` |
| `crates/winc-app/src/app/diff.rs` | content | fork 的暂存逻辑优先：`DiffTarget::StageHunk`／`StageLine`、`stage_diff_*`；采纳上游新增分支（折叠、水平滚动、`CommitGroup` 相关）|
| `crates/winc-app/src/view/diff.rs` | content | 同上；上游的折叠／滚动条渲染与 fork 的 ＋／− 暂存控件并存，浏览历史差异时隐藏暂存控件并跳过其命中区域 |

## 自动合并但要人工核对的点

自动合并成功不代表语义正确，合入后逐项检查：

- `crates/winc-app/src/app/lifecycle/window.rs`、`placement.rs`、`closing.rs`：上游改标题栏命中区域缩放、最大化恢复、双击缩放；fork 有 `deferred_close` 与 `closing.confirming` 两条关闭路径，确认两套语义没被合并成一条。
- `crates/winc-app/src/layout.rs`：上游项目徽章字符宽度改动（`a841a3d`）落在 fork 的 `layout.projects` 里，确认单窗口项目列表布局仍在。
- `crates/winc-app/src/app/input/mouse/*`：上游鼠标覆盖窗口判定（`1286244`）与 fork 的 `projects_hits` 命中区域共存。
- `crates/winc-app/src/app/snapshot/editor/format.rs`：上游格式化改动不得改掉 fork 的 `job` 编号。
- `crates/winc-app/src/app/git/*`、`snapshot/git/*`：上游 diff 折叠与 `e670653`（统一 diff 设置项）落在暂存区代码旁，确认 ＋／− 命中与 `commit.staging` 互斥未被打散。
- `xtask/src/*`、`Cargo.toml`：新 `crates/winc-vim` 需注册进 workspace 成员与 `winc-app` 依赖。

## 验证策略

1. 结构：`cargo metadata --no-deps` 列出 `winc-vim`；`ls crates/winc-vim/src` 非空。
2. 编译：`cargo clippy --workspace --all-targets`（含测试目标，能覆盖上游新增测试）。
3. 行为：`cargo test --workspace --no-fail-fast`；基线已知失败仅 `winc-pty --test windows` 的伪控制台用例（见 AGENTS.md）。
4. 不变量：按 `prd.md` 的清单人工核对代码路径（grep `StageHunk`、`projects_hits`、`deferred_close`、`formatting_done`）。

## 回滚

试验分支上任何阶段都可 `git merge --abort` 或 `git switch dev` 后删除 `merge/upstream-dev`；`dev` 只在全量测试通过后才合并，未通过的合并结果不进 `dev`。

## 取舍记录

冲突取舍与触碰的上游高频文件写进合并提交说明，规范层面的结论下沉到 `.trellis/spec/backend/*`（在 `../Winc-trellis` 提交）。

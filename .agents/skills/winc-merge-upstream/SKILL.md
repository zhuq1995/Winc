---
name: winc-merge-upstream
description: 维护 Winc fork 的上游检查、合并、合并后回归与本地 Windows 打包。用于“检查上游更新”“同步上游”“合并 upstream/main 或 upstream/dev”“合并后自有功能缺失”“给 Winc 打包”，按自有功能清单核对无冲突文件、设置默认值、菜单入口和窗口架构迁移，并核对当前分支的工具文件版本管理。
---

# Winc 上游同步

## 1. 确定范围与基线

1. 读取仓库通用 `AGENTS.md`、当前 Trellis 工作流、[维护规则](references/fork-maintenance.md) 与 [自有功能清单](references/fork-features.md) 全文。Fork 规则集中在本技能 references。按当前任务状态接续；创建任务遵守当前会话的授权。
2. 用户只要求检查时，执行获取远端引用、差异分析和报告。用户限制只用本地引用时跳过 fetch。合并、改代码、打包、安装与推送各按授权执行；仅打包时直接使用 [Windows 打包步骤](references/windows-packaging.md) 和 F20，记录构建 HEAD 与产物结果。
3. 记录当前工作区的分支、HEAD、脏文件和本地领先提交：

   ```bash
   git status --short --branch
   git remote -v
   git worktree list
   git rev-parse HEAD
   timeout 30 git fetch upstream --prune
   git rev-list --left-right --count HEAD...upstream/main
   git log --oneline HEAD..upstream/main
   git log --oneline upstream/main..upstream/dev
   ```

4. 默认目标为 `upstream/main`。仅在用户明确要求开发线时选 `upstream/dev`；若 main 已含 dev，使用 main。把目标解析成固定提交，记为 `target`，把合并前 dev 记为 `old_dev`，后续比较使用这两个值。
5. 读取当前 `VERSION` 与目标版本，统计文件变化并预演冲突：

   ```bash
   git diff --stat "$old_dev" "$target"
   git diff --name-status "$old_dev" "$target"
   git merge-tree --write-tree --name-only "$old_dev" "$target"
   ```

   `merge-tree` 返回 1 且列出冲突时，按预演结果处理；其他执行错误单独排查。只检查阶段到此输出报告，包括版本、提交数、主要功能、冲突范围和 F01–F20 的受影响项。

## 2. 建立功能保留表

1. 在任务中保存 `fork-preservation.md`，为 reference 中每个功能编号建立一行：

   | ID | 行为、入口及默认值 | 合并前实现与测试 | 目标上游的处理 | 合并后实现与检查结果 | 状态 |
   | --- | --- | --- | --- | --- | --- |

2. 状态使用「保留」「上游等价」「原有缺口」「待验证」「回归」。每行写具体文件、函数、命令及结果；不适用的配置默认值写明原因。
3. 先盘点现有 fork 差异，再看本轮上游变动，覆盖自动合并成功的文件：

   ```bash
   git log --no-merges --oneline "$target..$old_dev"
   git diff --name-status "$target" "$old_dev"
   git log --oneline "$old_dev..$target"
   ```

4. 逐项对照当前代码、相关提交和实际加载的测试。文件存在、符号存在、编译通过都只算定位依据。核对 `mod tests`、`cfg` 条件和测试枚举，检查默认配置与显式配置两条路径。
5. 把 reference 的当前复核项列入合并前基线。现有缺口保持可见；本轮引入的缺失标为回归。新增 fork 行为追加稳定编号，已有编号保留。

## 3. 执行合并

1. 开始修改前确认规划和任务已激活、应用工作区干净。已有本地改动按归属处理，保留用户尚未提交的内容。
2. 按维护规则选择分支：main 常规合入 dev；开发线在 `merge/upstream-dev` 试验分支操作。任务已批准 main 采用试验方案时使用 `merge/upstream-main`。已有同名分支先查归属，再选不冲突名称。以下为 main 试验方案：

   ```bash
   git switch -c merge/upstream-main dev
   git merge --no-ff --no-commit "$target"
   ```

3. 逐文件解决冲突并显式 `git add`。优先沿用上游结构，在调用点保留 fork 行为。上游等价实现接管时，删除重复实现，同时保留 fork 默认值、入口和安全约束。
4. 整文件取上游前，先对照保留表解释该文件的每项 fork 差异如何落地。`checkout --theirs` 只更新工作区；整文件覆盖还会删去自动合并保留的内容，需要重新核对并暂存。
5. 按 reference 重点检查：窗口句柄与渲染器成对交接、提示与异步保存、暂存区四处默认值、两个复制菜单、关闭入口、双语键集、CI 删除与工具文件跟踪。
6. 上游重写窗口生命周期时，按维护规则转为 tag 补丁集或向上游提交功能，把窗口交接的重新推导写入任务设计；后续范围遵守用户已作出的决定，交接未完成时保持试验分支。

## 4. 验收

1. 先确认冲突全部解决，再运行范围检查：

   ```bash
   git diff --name-only --diff-filter=U
   git diff --check
   cargo fmt --all --check
   cargo clippy --workspace --all-targets
   cargo test --workspace --no-fail-fast
   ```

   格式差异只修本次触碰范围，避免全仓无关改动。Windows Git Bash 的测试 PATH 含 Git 的 `cmd` 目录；临时仓库保持 LF，使用进程级配置或 fixture 配置，保留机器全局设置。
2. 用 `cargo test -p winc-app -- --list` 确认功能测试被加载，按 reference 运行对应检查。Windows 上的 `cargo check --target x86_64-unknown-linux-musl -p winc-app --all-targets` 只检查 Unix 分支编译；运行 Unix/macOS 行为测试需要对应环境。
3. 实际核对项目切换、终端继续运行、保存/退出确认、暂存索引与复制菜单。离屏快照只覆盖绘制或请求时，在保留表注明其范围。
4. 逐个核对 `$old_dev..$target` 中上游 `feat:` 提交在结果中有对应实现，兼顾上游新功能与 fork 行为。
5. 每个 F 编号给出结果。回归及本轮受影响的待验证项阻止合回 dev；原有缺口另列后续任务，保留基线证据。已知测试失败也需与基线比较，不能据测试名直接豁免。

## 5. 提交与交付

1. 在本轮合并分支提交，说明目标提交、冲突取舍、触碰的上游高频函数、F 编号及检查结果。使用试验分支时，所有检查完成后 `--no-ff` 合回 dev；确认 `git merge-base --is-ancestor "$target" dev`。
2. 按授权推送准确的分支；检查远端结果后才报告推送完成。失败时保留试验分支，按合并状态使用 `git merge --abort`；禁止对 dev rebase、force push 或用硬重置清除用户改动。
3. 更新本技能 references 的实现位置、默认值、维护规则与覆盖范围，以及相关 spec。Fork 说明集中在本技能中维护。`.trellis/`、`.agents/`、`.pi/` 随项目在当前开发分支直接版本管理；按任务范围显式暂存技能、规范、任务、journal 和应用文件。
4. 报告版本与提交、功能保留表、检查结果、未解决项和分支状态。检查结果逐项列「实际运行／只编译／静态定位／待验证」及平台；格式化与原生窗口相关项明确说明 Unix 条件、Windows 测试和 musl 编译各自的覆盖边界。用户要求打包时，按 Windows 打包步骤与 F20 检查包内主程序和当前构建的哈希，并提供产物路径。

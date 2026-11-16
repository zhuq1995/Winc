# 交付与检查

| 项目 | 当前结果 |
| --- | --- |
| 技能 | `.agents/skills/winc-merge-upstream/SKILL.md`，提交 `da7b4e4` |
| 功能清单 | F01–F20 共 20 个唯一编号；包含行为、入口、默认值、定位、检查要求和当前契约复核项 |
| 维护与打包 | `references/fork-maintenance.md`、`references/windows-packaging.md` |
| AGENTS.md | 仅保留 Trellis 管理区块；应用提交 `96ac7bd`，合入本地 dev 的提交 `7de8b86` |
| 结构 | quick_validate.py 返回 `Skill is valid!`；8 个相对文档链接可解析；两个评估各 6 条断言 |
| 首轮对照 | 两场景每配置各一次：使用技能 11/12，基线 7/12 |
| 针对性复核 | 合并证据场景每配置一次：使用技能 6/6，基线 3/6；包含 Unix 格式化条件与 Windows 覆盖范围 |
| 评估范围 | 只读本地源码与引用，应用测试和构建均未执行；时长及 tokens 缺少单次测量值 |
| 审阅报告 | `.agents/skills/winc-merge-upstream-workspace/iteration-1/review.html` 与 `iteration-2/review.html`，由 skill-creator 官方生成器生成 |
| 本地安装包 | `target/x86_64-pc-windows-msvc/release/bundle/nsis/Winc-0.1.733-windows-x64-setup.exe`，5751285 字节 |
| 本地便携包 | `target/x86_64-pc-windows-msvc/release/bundle/portable/Winc-0.1.733-windows-x64-portable.zip`，8050315 字节，13 文件 |
| 包完整性 | 两包 7z t 通过；包内主程序与 d54201a release 构建的 SHA-256 一致：`c7eae2296cb59b62771771b6412e3ae6a165399c55623636f68c3420208e8a53` |

两轮 AGENTS.md 基线不同，分数分别呈现；小样本只描述对应输出。references 中的现有源码缺口保持为后续复核项，本任务只迁移和维护工具文档。应用实现规范保持现状，Fork 维护规则和功能契约集中在技能 references。

技能及任务记录使用工具分支显式路径暂存；应用 dev 的本地提交保留在本机，推送目标为 `origin/chore/trellis-tooling`。前任务 journal 和其他归档目录保持各自归属。

<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->

## Fork 独有修改的维护

- 每次新增、修复或调整 fork 独有行为，都同步更新 [上游合并技能的功能保留清单](.agents/skills/winc-merge-upstream/references/fork-features.md)，作为该次修改的完成条件。
- 记录当前行为契约、入口与默认值、实现文件和函数、平台边界、回归测试或可执行检查步骤。已有功能更新原 F 编号，新增功能追加稳定编号；编号范围或合并检查流程变化时同步更新 [SKILL.md](.agents/skills/winc-merge-upstream/SKILL.md)。
- 上游合并按该清单核对全部受影响项，包括自动合并成功的文件；上游等价实现接管时仍保留行为、入口、默认值与回归要求。
- 技能与应用修改一起纳入本次交付和版本管理。排查过程与试错记录归 `docs/notes/`，功能保留清单只描述当前契约。

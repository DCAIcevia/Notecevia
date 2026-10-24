---
name: project-rules
description: 项目总规则入口。任何任务开始前加载。按需读同目录 references/ 下的开工、文档、代码、skill 四份细则。
---

# 项目规则

开工先声明四行：任务与完成条件、计划读的文件（≤5）、验证命令、可逆假设。

| 主题 | 文件 | 何时读 |
| --- | --- | --- |
| 开工与预算 | `references/workflow.md` | 每次任务开始 |
| 文档与禁用词 | `references/docs.md` | 写 README、skill、注释、说明文字 |
| 代码 | `references/code.md` | 写或改 Rust 代码 |
| skill 管理 | `references/skills.md` | 新增或修改 skill |

细则未读时也必须遵守三条：

- 该加载的 skill 没加载就动手，视为违规。
- 不改功能范围、不加依赖、不换 UI 方向，先问。
- 未验证不得称完成。

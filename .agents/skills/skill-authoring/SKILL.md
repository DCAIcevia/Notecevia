---
name: skill-authoring
description: 新增、修改或删除 .agents/skills 下的 skill 前加载。规定命名、description 写法、正文边界、长度上限、与 AGENTS.md 的同步要求、修改流程。
---

# Skill 编写与管控

规则只活在 skill 里。仓库不写 docs；`AGENTS.md` 只放定位、加载表与红线。

## 命名

- 目录名与 frontmatter `name` 一致，kebab-case，2–3 个词，说明管什么：`git-commit`、`ui-design`。
- 禁止 `rules`、`misc`、`notes`、`temp` 这类空名。

## description

- 这是唯一常驻可见的文字，必须说清两件事：什么时候加载、里面有什么。
- ≤60 字，中文，动词开头，不写"本 skill 介绍……"。
- 例：`任何界面相关改动前加载。规定视觉立场、禁用设计、令牌、布局、交互、四态、文案与验收。`

## 正文

- 只写判断依据与操作步骤，不写背景、理念、历史。
- 一律用 必须／禁止／允许 或祈使句。
- 一个 skill 一个主题；跨界内容拆成两个 skill。
- 单文件 ≤80 行；超了就拆。
- 不复制其他 skill 的内容，用名字引用。
- 表格优先于长段落。

## 与 AGENTS.md 同步

- 新增、删除或改名 skill，必须在同一次改动里更新 `AGENTS.md` 的加载表。否则它永远不会被加载。
- 加载表按「场景 → skill」写，一行一个场景。

## 流程

1. 先确认现有 skill 是否已覆盖，避免重复。
2. 写 description 与正文，≤80 行。
3. 更新 `AGENTS.md` 加载表。
4. 验证：`skill` 工具能按名字加载，内容与 description 一致。
5. 提交类型 `rules`，scope `skills`。

## 禁止

- 把 skill 写成文档：长篇解释、示例工程、截图。
- 一个 skill 覆盖多个互不相关的主题。
- 未更新加载表就新增 skill。
- 写会过期的版本号、临时路径、个人环境信息。


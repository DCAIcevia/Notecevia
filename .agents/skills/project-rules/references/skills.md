# Skill 管理

## 判据

新增前确认现有 skill 是否已覆盖。命名 kebab-case，目录名与 `name` 一致。

`description` ≤60 字，写清何时加载与内容。新增、改名、删除必须同次更新 `AGENTS.md` 加载表。

## 渐进式披露

单文件 ≤80 行。超过 30 行且含两个以上主题时拆分：SKILL.md 只留入口、索引表与硬约束（≤30 行），正文按主题移入同目录 `references/`。

索引表三列：主题、文件、何时读。一个主题一个文件，文件名 kebab-case，单个 reference ≤80 行。

单一主题的 skill 不拆，多一次读取没有收益。reference 之间不互相引用，不为凑数拆分。

## 流程

写 description 与正文，更新索引表，验证 `skill` 工具能按名字加载且内容与 description 一致。

提交类型 `rules`，scope `skills`。

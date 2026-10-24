# Notecevia

本地优先的桌面笔记应用。纯 Rust，单机离线，Markdown 文件即数据，不用 WebView。

## 规则入口

本文件只放定位、加载表与红线。规则只在 `.agents/skills/`，不凭记忆猜。

| 场景 | 先执行 |
| --- | --- |
| 任何任务开始 | `skill project-rules` |
| 提交、分支、发布 | `skill git-commit` |
| 改界面 | `skill ui-design` |
| 新建或修改 skill | `skill skill-creator` |

## 红线

- 该加载的 skill 没加载就动手，视为违规。
- 新依赖、新功能范围、新 UI 方向：先问，得到明确答复再做。
- 不读 `target/`，不整库扫描，不凭记忆写第三方 API。
- 缺信息先问；预算耗尽就停下，不自行脑补。
- 完成 = 门禁通过 + 四段汇报（改动／验证／风险／后续）。

## 结构

`crates/core` 领域逻辑｜`crates/store` 文件与索引｜`crates/ui` 界面｜`crates/app` 入口。

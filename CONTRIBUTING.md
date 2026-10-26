# 参与开发

仓库由 HanYang06 维护，`main` 受保护：禁止直接推送与强推，改动一律走分支与合并请求，合并方式只允许 merge commit；合并前必检项必须通过。单人维护期间由维护者自行合并，不要求外部批准。

## 开工前

读 `AGENTS.md`。规则只在 `.agents/skills/` 里，按 `AGENTS.md` 的加载表读对应 skill：总规则、提交与发布、界面设计、skill 写法各有入口。

工具链由仓库根的 `rust-toolchain.toml` 固定，rustup 按该文件选择工具链，本地与 CI 一致。

## 提交流程

1. 从 `main` 拉短分支，命名 `<type>/<scope>-<主题>`，例如 `feat/ui-board-canvas`。
2. 改动集中在分支上累积，达到里程碑的退出标准再合并；不按单次改动开合并请求。
3. 提交信息用 Conventional Commits，主题为中文，格式 `<type>(<scope>): <主题>`。
4. 推送前三条门禁全绿，并自测受影响的界面或数据路径。

## 贡献授权

提交的改动按 Apache-2.0 授权给本项目（根目录 `LICENSE` 已覆盖入站贡献），无需签署 CLA 或 DCO。改动带来新的第三方代码、字体或其他素材时，在同一个提交里更新 `THIRD_PARTY_LICENSES.md`。

## 依赖

新增依赖需同时满足：许可证在 `deny.toml` 的允许列表内，来源为 crates.io。CI 在每次推送与合并请求上审计依赖（漏洞、许可证、重复版本、来源），审计工作流每周再跑一次完整检查；列表外的许可证会使审计失败，确认可以接受后，把许可证加入 `deny.toml`。`advisories.ignore` 逐条记录上游尚未修复的 unmaintained 项，上游升级后删除。

`THIRD_PARTY_LICENSES.md` 由 `cargo about` 从当前依赖图生成，随二进制一起分发。依赖变化后重新生成并一并提交：

```bash
cargo install cargo-about --version 0.9.2 --locked --features cli
cargo about generate --locked about.hbs --output-file THIRD_PARTY_LICENSES.md
```

`about.toml` 的允许列表与 `deny.toml` 保持一致，出现列表外的许可证时生成失败；目标固定为 `x86_64-pc-windows-msvc`，换平台生成结果相同。发布流程会重新生成并比对，不一致即失败。

## 评审

- 评审看三件事：行为是否与路线图的退出标准一致、失败时数据是否安全、规则是否被遵守。
- 单人维护期间由维护者自审；评审意见落在合并请求的评论里，改完再合。

## 发布

推 `v*` 标签触发发布流程：先校验标签、版本与许可证，再在 Linux、Windows、macOS 上构建，生成 SBOM 与构建来源证明，最后创建 GitHub Release。标签里带 `-` 的版本自动标为预发布。

发布流程校验标签与 `notecevia-app` 的版本一致、标签提交在 `main` 上，不一致即失败；产物附 SHA256 校验和、构建来源证明与 SPDX SBOM。发布先创建草稿并上传资产，再转为正式发布；`v*` 标签由 ruleset 保护，不能删除或移动，`release` environment 只允许 `v*` 标签部署。

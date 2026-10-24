# 代码

## 分层

core 禁 IO、时钟、随机、UI 类型；ui 不依赖 store；越界违规。

## 错误

库内 `thiserror`；边界 `anyhow` + `context`；运行时禁 `unwrap`／`expect`；`panic!` 只用于违反不变式。

## 命名

词表 Note／Tag／Workspace／Trash；布尔 `is_`／`has_`；禁 `utils`、`helpers`、`common`。

## 规模

单文件 ≤400 行，单函数 ≤50 行。

## 测试

core 公开函数至少一例；store 用临时目录做集成测试，覆盖原子写、路径逃逸、未知字段保留、回收站；修复带回归测试。

## 门禁

`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`，三条全绿。

## 性能

冷启动 ≤500ms；1 万条列表 ≤1.5s；1 万条搜索 ≤100ms；输入延迟 ≤16ms；空闲内存 ≤200MB。

## 依赖

标准库优先；维护活跃或事实标准；MIT／Apache-2.0 兼容；先说用途与风险；直接依赖上限 40。

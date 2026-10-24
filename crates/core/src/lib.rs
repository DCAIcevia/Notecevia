#![forbid(unsafe_code)]

//! 领域模型与用例，不含 IO、时钟与界面类型。

/// 当前 crate 版本。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

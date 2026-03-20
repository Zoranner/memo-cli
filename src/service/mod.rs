//! 业务用例层：`commands` 为 CLI 子命令实现，`session` 统一打开本地 brain 与可选的 Embedding 客户端。

mod commands;
mod storage_dim;
mod time_range;

pub mod search;
pub mod session;

pub use commands::clear;
pub use commands::delete;
pub use commands::embed;
pub use commands::init;
pub use commands::list;
pub use commands::merge;
pub use commands::update;

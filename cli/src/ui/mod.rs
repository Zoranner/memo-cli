//! Terminal UI module
//!
//! 提供统一的终端输出格式化工具，遵循 Cargo 风格的输出规范。
//!
//! ## 设计原则
//!
//! - **统一格式**: 所有输出都通过 Output 对象格式化，保持一致的视觉风格
//! - **自动空行**: 方法内部自动处理空行，开发者无需手动管理
//! - **语义化**: 不同类型的输出使用不同的方法（info、warning、status 等）
//! - **易扩展**: 未来可以添加进度条、表格、颜色主题等功能
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use crate::ui::Output;
//!
//! let output = Output::new();
//!
//! // 显示数据库信息（自动添加空行）
//! output.database_info(&path, 100);
//!
//! // 显示进度状态
//! output.status("Loading", "model");
//!
//! // 显示警告（自动添加前后空行）
//! output.warning("API key not configured");
//!
//! // 用户确认
//! if output.confirm("yes")? {
//!     // 开始执行操作（自动添加前置空行）
//!     output.begin_operation("Clearing", "database");
//! }
//!
//! // 显示完成信息（自动添加前置空行）
//! output.finish("operation", "global");
//! ```

mod output;

pub use output::Output;

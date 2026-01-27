//! Memo Types - Common types and trait definitions
//!
//! This crate defines the data structures and storage interface
//! used across all memo implementations (local, remote, cloud).

pub mod models;
pub mod storage;

// Re-export commonly used types
pub use models::{MemoMetadata, MemoSection, Memory, MemoryBuilder, QueryResult, TimeRange};
pub use storage::{StorageBackend, StorageConfig};

//! Memo Local - LanceDB-based local storage backend
//!
//! This crate implements the StorageBackend trait using LanceDB
//! for local vector storage.

mod client;
mod db;

// Re-export the client (implements StorageBackend)
pub use client::LocalStorageClient;
// Re-export database metadata for service layer
pub use db::DatabaseMetadata;

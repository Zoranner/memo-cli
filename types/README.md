# memo-types

`memo-types` is the core type definition library for the memo-cli project, providing common data structures and interface definitions used across all modules.

## 📦 Design Philosophy

- **Zero Heavy Dependencies**: Only depends on basic libraries (serde, chrono, uuid, anyhow, async-trait)
- **Database Agnostic**: Type definitions are completely independent and don't depend on any specific storage implementation
- **Interface Abstraction**: Unified storage backend interface defined through traits
- **Extensibility**: Supports multiple storage backends (local, remote, cloud)

## 📂 Module Structure

```
types/
├── src/
│   ├── lib.rs        # Module entry, exports public API
│   ├── models.rs     # Core data structure definitions
│   └── storage.rs    # Storage backend trait definitions
└── Cargo.toml
```

## 🔍 Core Components

### Data Models (`models.rs`)

#### `Memory`
Core memory data structure representing a complete memory record:

```rust
pub struct Memory {
    pub id: String,              // Unique identifier in UUID format
    pub content: String,         // Memory content
    pub tags: Vec<String>,       // Tag list
    pub vector: Vec<f32>,        // Vector embedding
    pub source_file: Option<String>, // Optional source file
    pub created_at: DateTime<Utc>,   // Creation time
    pub updated_at: DateTime<Utc>,   // Update time
}
```

#### `QueryResult`
Query result structure used as return value for search and list operations:

```rust
pub struct QueryResult {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub score: Option<f32>,  // Similarity score (only present in search results)
}
```

#### Other Helper Types

- `TimeRange`: Time range filter
- `MemoryBuilder`: Builder pattern for constructing Memory
- `MemoSection`: Parsed section structure from Markdown
- `MemoMetadata`: Memory metadata

### Storage Interface (`storage.rs`)

#### `StorageBackend` trait

Defines the unified interface that all storage backends must implement:

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // Connection and initialization
    async fn connect(config: &StorageConfig) -> Result<Self> where Self: Sized;
    async fn init(&self) -> Result<()>;
    async fn exists(&self) -> Result<bool>;
    
    // Basic operations
    async fn insert(&self, memory: Memory) -> Result<()>;
    async fn insert_batch(&self, memories: Vec<Memory>) -> Result<()>;
    async fn update(&self, id: &str, content: String, vector: Vec<f32>, tags: Vec<String>) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;
    
    // Query operations
    async fn search_by_vector(&self, vector: Vec<f32>, limit: usize, threshold: f32, time_range: Option<TimeRange>) -> Result<Vec<QueryResult>>;
    async fn list(&self) -> Result<Vec<QueryResult>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<QueryResult>>;
    async fn find_similar(&self, vector: Vec<f32>, limit: usize, threshold: f32, exclude_id: Option<&str>) -> Result<Vec<QueryResult>>;
    
    // Metadata
    fn dimension(&self) -> usize;
    async fn count(&self) -> Result<usize>;
}
```

#### `StorageConfig`

Storage configuration structure with support for future extensions:

```rust
pub struct StorageConfig {
    pub path: String,      // Storage path (local) or URL (remote)
    pub dimension: usize,  // Vector dimension
    // Future extensions: url, auth, etc.
}
```

## 🎯 Use Cases

### Using in Other Crates

`memo-types` is referenced by other crates in the project:

```toml
[dependencies]
memo-types = { path = "../types" }
```

### Implementing Storage Backend

Any storage implementation needs to implement the `StorageBackend` trait:

```rust
use memo_types::{StorageBackend, StorageConfig, Memory, QueryResult};

pub struct MyStorage {
    // Your storage implementation
}

#[async_trait]
impl StorageBackend for MyStorage {
    async fn connect(config: &StorageConfig) -> Result<Self> {
        // Implement connection logic
    }
    
    // ... implement other methods
}
```

### Using Data Models

```rust
use memo_types::{Memory, MemoryBuilder, QueryResult};

// Create a memory
let memory = Memory::new(MemoryBuilder {
    content: "Memory content".to_string(),
    tags: vec!["rust".to_string(), "cli".to_string()],
    vector: vec![0.1, 0.2, 0.3], // 384-dimensional vector
    source_file: None,
});
```

## 🔗 Dependencies

```
memo-cli (CLI application)
    ↓
memo-local (Local storage implementation)
    ↓
memo-types (Type definitions) ← You are here
```

- **Depended upon**: `memo-cli`, `memo-local`, and other modules depend on this crate
- **No dependencies**: This crate doesn't depend on any other crates in the project, maintaining independence

## 📝 Design Principles

- **Separation of Concerns**: Type definitions separated from implementations for easier maintenance and testing
- **Vector Externalization**: Storage layer is not responsible for generating vectors, only storing and querying them
- **Async First**: All I/O operations use async/await
- **Error Handling**: Uses `anyhow::Result` to simplify error propagation
- **Testability**: Through trait abstraction, facilitates mocking and unit testing

## 🚀 Future Extensions

- Support for remote storage backends (HTTP API)
- Support for cloud storage (S3, MinIO, etc.)
- Add more metadata fields (author, version, etc.)
- Support custom vector dimensions
- Add transaction support

## 📄 License

Consistent with the main memo-cli project.

# Provider Configuration Design - 提供商配置设计

## 🎯 设计目标

将 API 提供商配置与应用配置分离，实现：
- 集中管理 API keys
- 一个 token 对应多个服务
- 易于添加和切换 provider
- 清晰的配置结构

## 📁 配置文件结构

```
~/.memo/
├── config.toml       # 主配置（引用 providers）
└── providers.toml    # 提供商配置（集中管理）
```

## 📐 配置设计

### providers.toml

```toml
# ============================================
# Provider 配置文件
# 集中管理所有 API 提供商的密钥和服务配置
# ============================================

# 阿里云 DashScope
[aliyun]
name = "阿里云 DashScope"
api_key = "sk-xxx"  # 一个 key 对应多个服务

  # Embedding 服务
  [aliyun.embed]
  type = "embed"
  base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
  model = "text-embedding-v4"
  dimension = 1024

  # Rerank 服务
  [aliyun.rerank]
  type = "rerank"
  base_url = "https://dashscope.aliyuncs.com/compatible-api/v1"
  model = "qwen3-rerank"

  # LLM 服务
  [aliyun.llm]
  type = "llm"
  base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
  model = "qwen-max"

# 智谱 AI
[zhipu]
name = "智谱 AI"
api_key = "xxx.yyy"

  [zhipu.rerank]
  type = "rerank"
  base_url = "https://open.bigmodel.cn/api/paas/v4"
  model = "rerank"

  [zhipu.embed]
  type = "embed"
  base_url = "https://open.bigmodel.cn/api/paas/v4"
  model = "embedding-3"
  dimension = 2048

# OpenAI
[openai]
name = "OpenAI"
api_key = "sk-xxx"

  [openai.embed]
  type = "embed"
  base_url = "https://api.openai.com/v1"
  model = "text-embedding-3-small"
  dimension = 1536

  [openai.llm]
  type = "llm"
  base_url = "https://api.openai.com/v1"
  model = "gpt-4"

# Ollama（本地）
[ollama]
name = "Ollama Local"
api_key = ""  # 本地不需要 key

  [ollama.embed]
  type = "embed"
  base_url = "http://localhost:11434"
  model = "bge-m3"
  dimension = 1024

  [ollama.rerank]
  type = "rerank"
  base_url = "http://localhost:11434"
  model = "bge-reranker-v2-m3"
```

### config.toml

```toml
# ============================================
# Memo 主配置文件
# ============================================

# 数据库路径（可选，默认: ~/.memo/brain）
# brain_path = "/path/to/your/brain"

# ============================================
# 服务配置（引用 providers.toml）
# ============================================

# Embedding 服务
embedding = "aliyun.embed"

# Rerank 服务
rerank = "aliyun.rerank"

# LLM 服务（用于多查询拆解等）
# llm = "aliyun.llm"

# ============================================
# 搜索配置
# ============================================

# 返回结果数量上限（默认: 10）
search_limit = 10

# 第一层搜索阈值（0.0-1.0，默认: 0.35）
similarity_threshold = 0.35

# 重复检测相似度阈值（0.0-1.0）
duplicate_threshold = 0.85
```

## 🏗️ 代码结构

### 1. 配置数据结构

```rust
// cli/src/config/providers.rs

/// Provider 配置
pub struct ProviderConfig {
    pub name: String,
    pub api_key: String,
    pub services: HashMap<String, ServiceConfig>,  // key: "embed", "rerank", "llm"
}

/// 服务配置
pub struct ServiceConfig {
    pub service_type: ServiceType,
    pub base_url: String,
    pub model: String,
    pub extra: HashMap<String, String>,  // 额外参数（如 dimension）
}

pub enum ServiceType {
    Embed,
    Rerank,
    Llm,
}

/// 所有 Provider 配置
pub struct ProvidersConfig {
    providers: HashMap<String, ProviderConfig>,
}

impl ProvidersConfig {
    /// 加载 providers.toml
    pub fn load() -> Result<Self>;
    
    /// 获取服务配置（如 "aliyun.embed"）
    pub fn get_service(&self, reference: &str) -> Result<&ServiceConfig>;
}
```

### 2. 应用配置

```rust
// cli/src/config/app_config.rs

pub struct AppConfig {
    pub brain_path: Option<PathBuf>,
    pub embedding_ref: String,      // "aliyun.embed"
    pub rerank_ref: String,          // "aliyun.rerank"
    pub llm_ref: Option<String>,     // "aliyun.llm"
    pub search_limit: usize,
    pub similarity_threshold: f32,
    pub duplicate_threshold: f32,
}

impl AppConfig {
    /// 加载 config.toml
    pub fn load() -> Result<Self>;
    
    /// 解析服务引用，返回完整配置
    pub fn resolve_service(&self, providers: &ProvidersConfig, service_ref: &str) 
        -> Result<ResolvedService>;
}

pub struct ResolvedService {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub extra: HashMap<String, String>,
}
```

### 3. Provider Trait

```rust
// cli/src/providers/rerank.rs

#[async_trait]
pub trait RerankProvider: Send + Sync {
    /// Rerank 文档
    async fn rerank(
        &self,
        query: &str,
        documents: &[&str],
        top_n: Option<usize>,
    ) -> Result<Vec<RerankItem>>;
}

/// 工厂函数
pub fn create_rerank_provider(config: &ResolvedService) -> Result<Box<dyn RerankProvider>> {
    // 根据 base_url 判断是哪个 provider
    if config.base_url.contains("dashscope.aliyuncs.com") {
        Ok(Box::new(AliyunRerankProvider::new(config)?))
    } else if config.base_url.contains("bigmodel.cn") {
        Ok(Box::new(ZhipuRerankProvider::new(config)?))
    } else if config.base_url.contains("localhost") {
        Ok(Box::new(OllamaRerankProvider::new(config)?))
    } else {
        anyhow::bail!("Unknown rerank provider: {}", config.base_url)
    }
}
```

## 🔄 迁移步骤

### 阶段 1：创建配置结构（不影响现有功能）
1. ✅ 创建 `cli/src/config/` 模块
2. ✅ 实现 `ProvidersConfig` 和 `AppConfig`
3. ✅ 生成 `providers.example.toml`

### 阶段 2：重构 Provider（向后兼容）
1. ✅ 创建 `cli/src/providers/` 模块
2. ✅ 实现 Provider traits
3. ✅ 保持旧配置格式兼容

### 阶段 3：切换到新配置
1. ✅ 修改服务使用新 Provider 接口
2. ✅ 更新文档
3. ✅ 废弃旧配置格式

## 💡 优势

1. **配置清晰**
   - Provider 配置集中管理
   - 应用配置简洁明了
   - 易于理解和维护

2. **易于扩展**
   - 添加新 provider 只需修改 providers.toml
   - 无需修改代码即可切换 provider

3. **安全性**
   - API keys 集中在一个文件
   - 可以单独备份和保护 providers.toml

4. **灵活性**
   - 一个 token 管理多个服务
   - 可以混合使用不同 provider（如 aliyun.embed + openai.llm）

5. **可测试性**
   - Provider trait 便于 mock
   - 配置加载逻辑独立

## 🎬 使用示例

### 切换 Rerank Provider

只需修改 `config.toml`：

```toml
# 使用阿里云
rerank = "aliyun.rerank"

# 切换到智谱（如果阿里云有问题）
# rerank = "zhipu.rerank"

# 切换到本地 Ollama
# rerank = "ollama.rerank"
```

### 混合使用多个 Provider

```toml
embedding = "aliyun.embed"    # 阿里云的 embedding（便宜）
rerank = "aliyun.rerank"      # 阿里云的 rerank（效果好）
llm = "openai.llm"            # OpenAI 的 LLM（质量高）
```

## 📝 待办事项

- [ ] 实现配置模块
- [ ] 实现 Provider traits
- [ ] 创建示例配置文件
- [ ] 重构现有服务
- [ ] 更新文档
- [ ] 编写测试

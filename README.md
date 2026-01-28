# 🧠 Memo - Long-Term Memory for Your AI Coding Assistant

> Help AI remember every conversation and accumulate development experience

[中文](docs/zh-CN/README.md) | English

---

## 💡 Why Memo?

- 💬 **AI keeps forgetting** - Explained a solution 3 days ago, have to explain again today
- 🔄 **Solving same problems** - Fixed a bug last week, similar one today, AI doesn't remember
- 📚 **Knowledge doesn't stick** - Every conversation is "one-off", valuable experience lost
- 🤝 **Team knowledge silos** - Everyone uses AI separately, can't share experience

---

## ⚡ Core Capabilities

| Capability | Description |
|------------|-------------|
| 🤖 **Auto Recording** | Say "remember this" in conversation, AI automatically saves to knowledge base |
| 🔍 **Smart Retrieval** | Say "how did we do this before", AI automatically searches related memories |
| ⏰ **Time Travel** | Quickly find development experience from "last week" or "last month" |
| 👥 **Team Collaboration** | Personal knowledge base or project-level sharing, flexible choice |
| 🔐 **Data Security** | Memory data stored entirely locally, fully under your control |
| 🌐 **Flexible Options** | Supports OpenAI, Ollama local, and more |

## 🚀 Quick Start

### Step 1: One-Click Install

**Windows (PowerShell):**
```powershell
irm https://memo.zoran.ink/install.ps1 | iex
```

**macOS/Linux:**
```bash
curl -fsSL https://memo.zoran.ink/install.sh | bash
```

### Step 2: Start Using

After installation, start using it directly in your AI coding tool:

```
You: Database connection keeps timing out, how to fix it?

AI: [Auto searches memory] memo search "database connection timeout" -n 5
    [No memories found]
    
    Database timeouts are usually caused by connection pool settings...
    Check max_connections, timeout parameters, etc.

You: Tried that, didn't work. Turns out the server firewall was blocking the port.
    Had to open port 3306 in Alibaba Cloud security group settings.

AI: Ah, that's an easy-to-miss issue! Let me record this:
    
    [Auto records] memo embed "Database Connection Timeout - Cloud Firewall Issue
    
    Problem: Database connection keeps timing out
    Cause: Cloud server doesn't open database port by default
    Solution: Login to cloud console → Security groups → Add rule → Open port 3306
    Note: Also check server's local firewall" --tags database,cloud,troubleshooting
    
    ✓ Recorded to memory

---

[Two weeks later, different project]

You: Can't connect to MySQL on the new project?

AI: [Auto searches memory] memo search "mysql connection" -n 5
    [Memory found]
    
    Had a similar issue before! If it's a cloud server, check security group settings:
    Login to cloud console → Security groups → Add rule → Open port 3306
    Also check the server's local firewall settings
```

## ⚙️ Configuration

### Config File Locations

- **Global config**: `~/.memo/config.toml` (recommended)
- **Local config**: `./.memo/config.toml` (project-specific)

### Priority Order

Command-line args > Local config > Global config > Defaults

### Configuration Parameters

| Parameter | Required | Description | Default |
|-----------|:--------:|-------------|---------|
| `embedding_api_key` | ✅ | API key | - |
| `embedding_model` | ✅ | Model name | - |
| `embedding_base_url` | ❌ | API endpoint | `https://api.openai.com/v1` |
| `embedding_provider` | ❌ | Provider type | Auto-inferred |
| `embedding_dimension` | ❌ | Vector dimension | Auto-inferred |
| `similarity_threshold` | ❌ | Search similarity threshold (0-1) | `0.7` |
| `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |

### Supported API Types

**OpenAI-compatible API (default):**
```toml
embedding_api_key = "sk-..."
embedding_model = "text-embedding-3-small"
# embedding_base_url = "https://api.example.com/v1"  # Optional
```

**Ollama local deployment:**
```toml
embedding_base_url = "http://localhost:11434/api"
embedding_api_key = ""  # No key needed for local
embedding_model = "nomic-embed-text"
```

## 📜 License

MIT

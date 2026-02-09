use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 服务类型
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Embed,
    Rerank,
    Llm,
}

/// 服务配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    #[serde(rename = "type")]
    pub service_type: ServiceType,
    pub base_url: String,
    pub model: String,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

/// Provider 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key: String,
    #[serde(flatten)]
    pub services: HashMap<String, ServiceConfig>,
}

/// 所有 Provider 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProvidersConfig {
    #[serde(flatten)]
    providers: HashMap<String, ProviderConfig>,
}

impl ProvidersConfig {
    /// 加载 providers.toml
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            anyhow::bail!(
                "Providers configuration not found at: {}\nPlease create it from providers.example.toml",
                config_path.display()
            );
        }

        let content = std::fs::read_to_string(&config_path).with_context(|| {
            format!("Failed to read providers config: {}", config_path.display())
        })?;

        let config: Self = toml::from_str(&content).with_context(|| {
            format!(
                "Failed to parse providers config: {}",
                config_path.display()
            )
        })?;

        tracing::debug!("Loaded providers config from: {}", config_path.display());
        tracing::debug!(
            "Available providers: {:?}",
            config.providers.keys().collect::<Vec<_>>()
        );

        Ok(config)
    }

    /// 获取配置文件路径
    fn get_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".memo").join("providers.toml"))
    }

    /// 获取服务配置（如 "aliyun.embed"）
    pub fn get_service(&self, reference: &str) -> Result<ResolvedService> {
        let parts: Vec<&str> = reference.split('.').collect();

        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid service reference: '{}'. Expected format: 'provider.service' (e.g., 'aliyun.embed')",
                reference
            );
        }

        let provider_name = parts[0];
        let service_name = parts[1];

        let provider = self
            .providers
            .get(provider_name)
            .with_context(|| format!("Provider '{}' not found in providers.toml", provider_name))?;

        let service = provider.services.get(service_name).with_context(|| {
            format!(
                "Service '{}' not found in provider '{}'",
                service_name, provider_name
            )
        })?;

        Ok(ResolvedService {
            api_key: provider.api_key.clone(),
            base_url: service.base_url.clone(),
            model: service.model.clone(),
            extra: service.extra.clone(),
        })
    }
}

/// 解析后的服务配置
#[derive(Debug, Clone)]
pub struct ResolvedService {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub extra: HashMap<String, toml::Value>,
}

impl ResolvedService {
    /// 获取整数类型的额外参数
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|v| v.as_integer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_providers_config() {
        let toml_str = r#"
[aliyun]
name = "阿里云 DashScope"
api_key = "sk-test"

  [aliyun.embed]
  type = "embed"
  base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
  model = "text-embedding-v4"
  dimension = 1024

  [aliyun.rerank]
  type = "rerank"
  base_url = "https://dashscope.aliyuncs.com/compatible-api/v1"
  model = "qwen3-rerank"

[zhipu]
name = "智谱 AI"
api_key = "xxx.yyy"

  [zhipu.embed]
  type = "embed"
  base_url = "https://open.bigmodel.cn/api/paas/v4"
  model = "embedding-3"
  dimension = 2048
        "#;

        let config: ProvidersConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.providers.len(), 2);
        assert!(config.providers.contains_key("aliyun"));
        assert!(config.providers.contains_key("zhipu"));

        let resolved = config.get_service("aliyun.embed").unwrap();
        assert_eq!(resolved.api_key, "sk-test");
        assert_eq!(resolved.model, "text-embedding-v4");
        assert_eq!(resolved.get_int("dimension"), Some(1024));
    }
}

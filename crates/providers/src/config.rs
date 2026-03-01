/// Provider 配置
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider_name: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub dimension: Option<usize>,
}

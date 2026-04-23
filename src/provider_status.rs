use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const PROVIDER_RUNTIME_FILE: &str = "provider-runtime.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub(crate) struct ProviderRuntimeSummary {
    #[serde(default)]
    pub statuses: Vec<ProviderCapabilityStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProviderCapabilityStatus {
    pub capability: String,
    pub provider_ref: String,
    pub status: ProviderHealth,
    pub consecutive_failures: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderHealth {
    Ok,
    Degraded,
}

impl ProviderHealth {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderRuntimeRecorder {
    path: PathBuf,
}

impl ProviderRuntimeRecorder {
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(PROVIDER_RUNTIME_FILE),
        }
    }

    pub(crate) fn record_success(&self, capability: &str, provider_ref: &str) -> Result<()> {
        self.update(capability, provider_ref, |status| {
            status.status = ProviderHealth::Ok;
            status.consecutive_failures = 0;
            status.last_error = None;
            status.updated_at = Utc::now();
        })
    }

    pub(crate) fn record_failure(
        &self,
        capability: &str,
        provider_ref: &str,
        error: &anyhow::Error,
    ) -> Result<()> {
        let message = error.to_string();
        self.update(capability, provider_ref, |status| {
            status.status = ProviderHealth::Degraded;
            status.consecutive_failures = status.consecutive_failures.saturating_add(1).max(1);
            status.last_error = Some(message.clone());
            status.updated_at = Utc::now();
        })
    }

    fn update(
        &self,
        capability: &str,
        provider_ref: &str,
        mutate: impl FnOnce(&mut ProviderCapabilityStatus),
    ) -> Result<()> {
        let _guard = provider_runtime_lock()
            .lock()
            .expect("provider runtime mutex poisoned");
        let mut summary = read_summary_file(&self.path).unwrap_or_default();
        let now = Utc::now();
        let status = match summary
            .statuses
            .iter_mut()
            .find(|item| item.capability == capability)
        {
            Some(existing) => existing,
            None => {
                summary.statuses.push(ProviderCapabilityStatus {
                    capability: capability.to_string(),
                    provider_ref: provider_ref.to_string(),
                    status: ProviderHealth::Ok,
                    consecutive_failures: 0,
                    last_error: None,
                    updated_at: now,
                });
                summary
                    .statuses
                    .last_mut()
                    .expect("provider status just inserted")
            }
        };
        status.provider_ref = provider_ref.to_string();
        mutate(status);
        summary.statuses.sort_by(|left, right| {
            left.capability
                .cmp(&right.capability)
                .then(left.provider_ref.cmp(&right.provider_ref))
        });
        write_summary_file(&self.path, &summary)
    }
}

pub(crate) fn load_provider_runtime_summary(data_dir: &Path) -> ProviderRuntimeSummary {
    let path = data_dir.join(PROVIDER_RUNTIME_FILE);
    match read_summary_file(&path) {
        Ok(summary) => summary,
        Err(_) if !path.exists() => ProviderRuntimeSummary::default(),
        Err(error) => ProviderRuntimeSummary {
            statuses: Vec::new(),
            read_error: Some(error.to_string()),
        },
    }
}

fn read_summary_file(path: &Path) -> Result<ProviderRuntimeSummary> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut summary: ProviderRuntimeSummary = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    summary.statuses.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then(left.provider_ref.cmp(&right.provider_ref))
    });
    Ok(summary)
}

fn write_summary_file(path: &Path, summary: &ProviderRuntimeSummary) -> Result<()> {
    let raw = serde_json::to_string_pretty(summary)?;
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))
}

fn provider_runtime_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use tempfile::TempDir;

    use super::{load_provider_runtime_summary, ProviderHealth, ProviderRuntimeRecorder};

    #[test]
    fn recorder_tracks_degraded_then_recovered_provider_status() -> Result<()> {
        let temp = TempDir::new()?;
        let recorder = ProviderRuntimeRecorder::new(temp.path());

        recorder.record_failure("embedding", "openai.embed", &anyhow::anyhow!("timeout"))?;
        let degraded = load_provider_runtime_summary(temp.path());
        let degraded_status = degraded
            .statuses
            .iter()
            .find(|status| status.capability == "embedding")
            .expect("expected embedding status");
        assert_eq!(degraded_status.status, ProviderHealth::Degraded);
        assert_eq!(degraded_status.consecutive_failures, 1);
        assert_eq!(degraded_status.last_error.as_deref(), Some("timeout"));

        recorder.record_success("embedding", "openai.embed")?;
        let recovered = load_provider_runtime_summary(temp.path());
        let recovered_status = recovered
            .statuses
            .iter()
            .find(|status| status.capability == "embedding")
            .expect("expected embedding status");
        assert_eq!(recovered_status.status, ProviderHealth::Ok);
        assert_eq!(recovered_status.consecutive_failures, 0);
        assert!(recovered_status.last_error.is_none());
        Ok(())
    }

    #[test]
    fn load_provider_runtime_summary_exposes_read_errors() -> Result<()> {
        let temp = TempDir::new()?;
        fs::write(temp.path().join("provider-runtime.json"), "{not-json")?;

        let summary = load_provider_runtime_summary(temp.path());

        assert!(summary.statuses.is_empty());
        assert!(summary
            .read_error
            .as_deref()
            .is_some_and(|error| error.contains("failed to parse")));
        Ok(())
    }
}

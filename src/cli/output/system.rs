use std::path::Path;

use anyhow::Result;
use memo_engine::{DreamReport, IndexStatus, RestoreReport, SystemState};

use crate::{config, providers::status};

use super::common::{created_label, render_json_or_text};

pub(crate) fn render_awaken_result(
    data_dir: &Path,
    config_dir: &Path,
    report: &config::InitReport,
) -> String {
    format!(
        "Awakened memory space at {}\nconfig_dir: {}\nconfig.toml: {}\nproviders.toml: {}",
        data_dir.display(),
        config_dir.display(),
        created_label(report.config_created),
        created_label(report.providers_created),
    )
}

pub(crate) fn render_dream_report(report: &DreamReport, full: bool, json: bool) -> Result<String> {
    if json {
        let payload = serde_json::json!({
            "mode": if full { "full" } else { "standard" },
            "dream": report,
        });
        return render_json_or_text(&payload, "", true);
    }

    Ok(format!(
        "Dream {}complete\npasses_run: {}\nstructured_episodes: {}\nstructured_entities: {}\nstructured_facts: {}\nextraction_failures: {}\npromoted_to_l2: {}\npromoted_to_l3: {}\ndowngraded: {}\narchived: {}\ninvalidated: {}",
        if full { "(full) " } else { "" },
        report.passes_run,
        report.structured_episodes,
        report.structured_entities,
        report.structured_facts,
        report.extraction_failures,
        report.promoted_to_l2,
        report.promoted_to_l3,
        report.downgraded_records,
        report.archived_records,
        report.invalidated_records,
    ))
}

pub(crate) fn render_state(
    state: &SystemState,
    provider_runtime: &status::ProviderRuntimeSummary,
    json: bool,
) -> Result<String> {
    if json {
        return render_json_or_text(
            &serde_json::json!({
                "state": state,
                "provider_runtime": provider_runtime,
            }),
            "",
            true,
        );
    }

    Ok(format!(
        "State\nrecords: episodes={} entities={} facts={} edges={}\nlayers: l1={} l2={} l3={} archived={} invalidated={}\nl3_cached: {}\ntext_index: {}\nvector_index: {}\nprovider_runtime: {}",
        state.episode_count,
        state.entity_count,
        state.fact_count,
        state.edge_count,
        state.layers.l1,
        state.layers.l2,
        state.layers.l3,
        state.layers.archived,
        state.layers.invalidated,
        state.l3_cached,
        index_summary(&state.text_index),
        index_summary(&state.vector_index),
        provider_runtime_summary(provider_runtime),
    ))
}

pub(crate) fn render_restore_report(
    report: &RestoreReport,
    full: bool,
    json: bool,
) -> Result<String> {
    if json {
        let payload = serde_json::json!({
            "mode": if full { "full" } else { "standard" },
            "restore": report,
        });
        return render_json_or_text(&payload, "", true);
    }

    Ok(format!(
        "Restore {}complete\ntext_documents: {}\nvector_documents: {}",
        if full { "(full) " } else { "" },
        report.text_documents,
        report.vector_documents,
    ))
}

fn index_summary(index: &IndexStatus) -> String {
    let mut segments = vec![format!("{} docs={}", index.status, index.doc_count)];
    if index.pending_updates > 0 {
        segments.push(format!("pending_updates={}", index.pending_updates));
    }
    if index.failed_updates > 0 {
        segments.push(format!("failed_updates={}", index.failed_updates));
    }
    if index.failed_attempts_max > 0 {
        segments.push(format!("failed_attempts_max={}", index.failed_attempts_max));
    }
    if let Some(last_error) = index.last_error.as_deref() {
        segments.push(format!("last_error={last_error}"));
    }
    if let Some(detail) = index.detail.as_deref() {
        segments.push(format!("detail={detail}"));
    }
    segments.join(" ")
}

fn provider_runtime_summary(summary: &status::ProviderRuntimeSummary) -> String {
    if let Some(read_error) = summary.read_error.as_deref() {
        return format!("unavailable detail={read_error}");
    }

    if summary.statuses.is_empty() {
        return "idle".to_string();
    }

    summary
        .statuses
        .iter()
        .map(|status| {
            let mut segments = vec![
                format!("{}={}", status.capability, status.status.as_str()),
                format!("provider={}", status.provider_ref),
            ];
            if status.consecutive_failures > 0 {
                segments.push(format!(
                    "consecutive_failures={}",
                    status.consecutive_failures
                ));
            }
            if let Some(last_error) = status.last_error.as_deref() {
                segments.push(format!("last_error={last_error}"));
            }
            segments.join(" ")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

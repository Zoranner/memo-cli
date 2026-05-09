use std::path::Path;

use anyhow::Result;
use memo_engine::{DreamReport, IndexStatus, SystemState};
use serde::Serialize;

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

    let mut output = format!(
        "Dream {}complete\npasses_run: {}\nunstructured_l1: {}\nunstructured_l2: {}\nstructured_episodes: {}\nstructured_entities: {}\nstructured_facts: {}\nextraction_failures: {}\nprovider_extraction_calls: {}\nprovider_embedding_calls: {}\npromoted_to_l2: {}\npromoted_to_l3: {}\ndowngraded: {}\narchived: {}\ninvalidated: {}\npinned_skipped: {}",
        if full { "(full) " } else { "" },
        report.passes_run,
        report.unstructured_l1,
        report.unstructured_l2,
        report.structured_episodes,
        report.structured_entities,
        report.structured_facts,
        report.extraction_failures,
        report.provider_calls.extraction_calls,
        report.provider_calls.embedding_calls,
        report.promoted_to_l2,
        report.promoted_to_l3,
        report.downgraded_records,
        report.archived_records,
        report.invalidated_records,
        report.pinned_skipped,
    );
    if !report.maintenance_notes.is_empty() {
        output.push_str("\nnotes: ");
        output.push_str(&report.maintenance_notes.join("; "));
    }
    if report.derived_repairs > 0 || report.derived_refreshes > 0 {
        output.push_str(&format!(
            "\nderived_repairs: {}\nderived_refreshes: {}\nderived_text_documents: {}\nderived_vector_documents: {}",
            report.derived_repairs,
            report.derived_refreshes,
            report.derived_text_documents,
            report.derived_vector_documents
        ));
    }
    Ok(output)
}

pub(crate) fn render_state(
    state: &SystemState,
    provider_runtime: &status::ProviderRuntimeSummary,
    provider_readiness: &status::ProviderReadinessSummary,
    json: bool,
) -> Result<String> {
    let summary = StateOutput::from_inputs(state, provider_runtime, provider_readiness);
    if json {
        return render_json_or_text(&summary, "", true);
    }

    Ok(format!(
        "status: {}\nmessage: {}\nnext: {}",
        summary.status, summary.message, summary.next
    ))
}

#[derive(Debug, Serialize)]
struct StateOutput<'a> {
    status: &'static str,
    message: &'static str,
    next: &'static str,
    diagnostics: StateDiagnostics<'a>,
}

#[derive(Debug, Serialize)]
struct StateDiagnostics<'a> {
    internal_reasons: Vec<&'static str>,
    state: &'a SystemState,
    provider_runtime: &'a status::ProviderRuntimeSummary,
    provider_readiness: &'a status::ProviderReadinessSummary,
}

impl<'a> StateOutput<'a> {
    fn from_inputs(
        state: &'a SystemState,
        provider_runtime: &'a status::ProviderRuntimeSummary,
        provider_readiness: &'a status::ProviderReadinessSummary,
    ) -> Self {
        let mut internal_reasons = internal_reasons(state, provider_runtime, provider_readiness);
        let needs_setup = internal_reasons.contains(&"provider_not_ready");
        let needs_dream = internal_reasons.iter().any(|reason| {
            matches!(
                *reason,
                "needs_structure" | "needs_vectors" | "sync_needed" | "full_refresh_needed"
            )
        });
        let (status, message, next) = if needs_setup {
            ("needs_setup", "需要先配置 provider", "configure provider")
        } else if needs_dream {
            ("needs_dream", "有新内容需要整理", "memo dream")
        } else {
            ("ready", "记忆系统已就绪", "none")
        };

        internal_reasons.sort_unstable();
        internal_reasons.dedup();

        Self {
            status,
            message,
            next,
            diagnostics: StateDiagnostics {
                internal_reasons,
                state,
                provider_runtime,
                provider_readiness,
            },
        }
    }
}

fn internal_reasons(
    state: &SystemState,
    provider_runtime: &status::ProviderRuntimeSummary,
    provider_readiness: &status::ProviderReadinessSummary,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();

    if required_provider_not_ready(provider_readiness) {
        reasons.push("provider_not_ready");
    }
    if provider_runtime.read_error.is_some() {
        reasons.push("provider_runtime_unavailable");
    }
    if state.unstructured_l1 > 0 || state.unstructured_l2 > 0 {
        reasons.push("needs_structure");
    }
    if state.episode_count > 0 && state.structured_total == 0 {
        reasons.push("needs_structure");
    }
    if state.structured_total > 0 && state.vector_index.doc_count == 0 {
        reasons.push("needs_vectors");
    }
    if index_needs_sync(&state.text_index) || index_needs_sync(&state.vector_index) {
        reasons.push("sync_needed");
    }
    if index_needs_full_refresh(&state.text_index) || index_needs_full_refresh(&state.vector_index)
    {
        reasons.push("full_refresh_needed");
    }

    reasons
}

fn required_provider_not_ready(summary: &status::ProviderReadinessSummary) -> bool {
    let Some(extraction) = summary
        .capabilities
        .iter()
        .find(|capability| capability.capability == "extraction")
    else {
        return true;
    };

    matches!(
        extraction.status,
        status::ProviderReadiness::NotConfigured
            | status::ProviderReadiness::PlaceholderKey
            | status::ProviderReadiness::Degraded
    )
}

fn index_needs_sync(index: &IndexStatus) -> bool {
    index.pending_updates > 0 || index.failed_updates > 0 || index.status == "pending"
}

fn index_needs_full_refresh(index: &IndexStatus) -> bool {
    index.status == "failed" || index.failed_attempts_max > 0 || index.last_error.is_some()
}

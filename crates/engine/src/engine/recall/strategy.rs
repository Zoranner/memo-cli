use super::*;

#[cfg(test)]
pub(super) fn should_auto_escalate_to_deep_search(result: &RecallResultSet) -> bool {
    matches!(
        select_recall_search_strategy(result),
        RecallSearchStrategy::Deep
    )
}
pub(super) fn select_recall_search_strategy(result: &RecallResultSet) -> RecallSearchStrategy {
    const WEAK_SINGLE_RESULT_SCORE_THRESHOLD: f32 = 0.9;
    const AMBIGUOUS_SCORE_GAP_THRESHOLD: f32 = 0.25;

    let Some(first) = result.results.first() else {
        return RecallSearchStrategy::Deep;
    };
    if has_decisive_reason(&first.reasons) {
        return RecallSearchStrategy::Fast;
    }

    if result.results.len() == 1 {
        return if first.score <= WEAK_SINGLE_RESULT_SCORE_THRESHOLD {
            RecallSearchStrategy::Deep
        } else {
            RecallSearchStrategy::Fast
        };
    }

    let second = &result.results[1];
    let score_gap = (first.score - second.score).abs();
    if score_gap <= AMBIGUOUS_SCORE_GAP_THRESHOLD {
        RecallSearchStrategy::Deep
    } else {
        RecallSearchStrategy::Fast
    }
}
fn has_decisive_reason(reasons: &[RecallReason]) -> bool {
    reasons.iter().any(|reason| {
        matches!(
            reason,
            RecallReason::L0 | RecallReason::L3 | RecallReason::Exact | RecallReason::Alias
        )
    })
}

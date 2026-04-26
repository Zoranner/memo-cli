use serde::{Deserialize, Serialize};

use super::EvalReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalGateReport {
    pub profile: String,
    pub passed: bool,
    pub failures: Vec<EvalGateViolation>,
    pub warnings: Vec<EvalGateWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalGateViolation {
    pub metric: String,
    pub expected: f32,
    pub actual: f32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalGateWarning {
    pub metric: String,
    pub expected: f32,
    pub actual: f32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RecallQualityGateProfile {
    pub name: String,
    pub minimums: Vec<EvalMetricMinimum>,
    pub warning_minimums: Vec<EvalMetricMinimum>,
    pub warning_maximums: Vec<EvalMetricMaximum>,
}

impl RecallQualityGateProfile {
    pub fn synthetic_quality() -> Self {
        Self {
            name: "synthetic_quality".to_string(),
            minimums: vec![
                EvalMetricMinimum::new("recall_at_1", 0.60),
                EvalMetricMinimum::new("recall_at_5", 0.85),
                EvalMetricMinimum::new("mrr", 0.70),
                EvalMetricMinimum::new("source_mrr", 0.70),
                EvalMetricMinimum::new("clean_hit_rate", 0.50),
                EvalMetricMinimum::new("abstention_correctness", 1.0),
            ],
            warning_minimums: vec![EvalMetricMinimum::new("forbidden_correctness", 1.0)],
            warning_maximums: vec![EvalMetricMaximum::new("mean_duplicate_rate", 0.20)],
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalMetricMinimum {
    pub metric: String,
    pub minimum: f32,
}

impl EvalMetricMinimum {
    pub fn new(metric: impl Into<String>, minimum: f32) -> Self {
        Self {
            metric: metric.into(),
            minimum,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalMetricMaximum {
    pub metric: String,
    pub maximum: f32,
}

impl EvalMetricMaximum {
    pub fn new(metric: impl Into<String>, maximum: f32) -> Self {
        Self {
            metric: metric.into(),
            maximum,
        }
    }
}

pub fn evaluate_recall_quality_gate(
    report: &EvalReport,
    profile: RecallQualityGateProfile,
) -> EvalGateReport {
    let mut failures = Vec::new();
    for minimum in &profile.minimums {
        let actual = eval_metric_value(report, &minimum.metric);
        if actual < minimum.minimum {
            failures.push(EvalGateViolation {
                metric: minimum.metric.clone(),
                expected: minimum.minimum,
                actual,
                message: format!(
                    "{} must be at least {:.3}, got {:.3}",
                    minimum.metric, minimum.minimum, actual
                ),
            });
        }
    }

    let mut warnings = Vec::new();
    for minimum in &profile.warning_minimums {
        let actual = eval_metric_value(report, &minimum.metric);
        if actual < minimum.minimum {
            warnings.push(EvalGateWarning {
                metric: minimum.metric.clone(),
                expected: minimum.minimum,
                actual,
                message: format!(
                    "{} is below {:.3}; this is a known retrieval risk, got {:.3}",
                    minimum.metric, minimum.minimum, actual
                ),
            });
        }
    }
    for maximum in &profile.warning_maximums {
        let actual = eval_metric_value(report, &maximum.metric);
        if actual > maximum.maximum {
            warnings.push(EvalGateWarning {
                metric: maximum.metric.clone(),
                expected: maximum.maximum,
                actual,
                message: format!(
                    "{} is above {:.3}; this is a known retrieval risk, got {:.3}",
                    maximum.metric, maximum.maximum, actual
                ),
            });
        }
    }

    EvalGateReport {
        profile: profile.name,
        passed: failures.is_empty(),
        failures,
        warnings,
    }
}

fn eval_metric_value(report: &EvalReport, metric: &str) -> f32 {
    match metric {
        "recall_at_1" => report.recall_at_1,
        "recall_at_5" => report.recall_at_5,
        "source_recall_at_1" => report.source_recall_at_1,
        "source_recall_at_5" => report.source_recall_at_5,
        "mrr" => report.mrr,
        "source_mrr" => report.source_mrr,
        "expected_hit_rate" => report.expected_hit_rate,
        "clean_hit_rate" => report.clean_hit_rate,
        "successful_case_rate" => report.successful_case_rate,
        "precision_at_1" => report.precision_at_1,
        "precision_at_5" => report.precision_at_5,
        "clean_precision_at_5" => report.clean_precision_at_5,
        "forbidden_rate" => report.forbidden_rate,
        "noise_hit_rate" => report.noise_hit_rate,
        "mean_source_diversity" => report.mean_source_diversity,
        "mean_duplicate_rate" => report.mean_duplicate_rate,
        "abstention_correctness" => report.abstention_correctness,
        "forbidden_correctness" => report.forbidden_correctness,
        _ => panic!("unknown eval metric: {metric}"),
    }
}

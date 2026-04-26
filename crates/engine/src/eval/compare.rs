use serde::{Deserialize, Serialize};

use super::EvalReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalComparison {
    pub passed: bool,
    pub regressions: Vec<EvalRegression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRegression {
    pub metric: String,
    pub baseline: f32,
    pub current: f32,
    pub delta: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct EvalCompareOptions {
    pub max_quality_drop: f32,
    pub max_rate_increase: f32,
    pub max_latency_ratio: f32,
}

impl Default for EvalCompareOptions {
    fn default() -> Self {
        Self {
            max_quality_drop: 0.05,
            max_rate_increase: 0.05,
            max_latency_ratio: 1.25,
        }
    }
}

pub fn compare_eval_reports(
    baseline: &EvalReport,
    current: &EvalReport,
    options: EvalCompareOptions,
) -> EvalComparison {
    let mut regressions = Vec::new();
    push_quality_regression(
        &mut regressions,
        "recall_at_1",
        baseline.recall_at_1,
        current.recall_at_1,
        options.max_quality_drop,
    );
    push_quality_regression(
        &mut regressions,
        "recall_at_5",
        baseline.recall_at_5,
        current.recall_at_5,
        options.max_quality_drop,
    );
    push_quality_regression(
        &mut regressions,
        "clean_hit_rate",
        baseline.clean_hit_rate,
        current.clean_hit_rate,
        options.max_quality_drop,
    );
    push_quality_regression(
        &mut regressions,
        "successful_case_rate",
        baseline.successful_case_rate,
        current.successful_case_rate,
        options.max_quality_drop,
    );
    push_rate_regression(
        &mut regressions,
        "forbidden_rate",
        baseline.forbidden_rate,
        current.forbidden_rate,
        options.max_rate_increase,
    );
    push_rate_regression(
        &mut regressions,
        "mean_duplicate_rate",
        baseline.mean_duplicate_rate,
        current.mean_duplicate_rate,
        options.max_rate_increase,
    );
    if baseline.timing.total_ms > 0.0
        && current.timing.total_ms > baseline.timing.total_ms * options.max_latency_ratio as f64
    {
        regressions.push(EvalRegression {
            metric: "total_ms".to_string(),
            baseline: baseline.timing.total_ms as f32,
            current: current.timing.total_ms as f32,
            delta: (current.timing.total_ms - baseline.timing.total_ms) as f32,
        });
    }

    EvalComparison {
        passed: regressions.is_empty(),
        regressions,
    }
}

fn push_quality_regression(
    regressions: &mut Vec<EvalRegression>,
    metric: &str,
    baseline: f32,
    current: f32,
    max_drop: f32,
) {
    if baseline - current > max_drop {
        regressions.push(EvalRegression {
            metric: metric.to_string(),
            baseline,
            current,
            delta: current - baseline,
        });
    }
}

fn push_rate_regression(
    regressions: &mut Vec<EvalRegression>,
    metric: &str,
    baseline: f32,
    current: f32,
    max_increase: f32,
) {
    if current - baseline > max_increase {
        regressions.push(EvalRegression {
            metric: metric.to_string(),
            baseline,
            current,
            delta: current - baseline,
        });
    }
}

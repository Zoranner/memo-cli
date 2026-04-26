use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

#[path = "support/quality.rs"]
mod quality_support;

use quality_support::{
    check_quality_gate, parse_eval_dataset, print_recall_quality_report, run_eval_dataset,
    write_eval_report_artifact, SYNTHETIC_QUALITY, SYNTHETIC_SMOKE,
};

fn recall_quality(c: &mut Criterion) {
    let baseline = run_eval_dataset(parse_eval_dataset(SYNTHETIC_QUALITY));
    write_eval_report_artifact("recall_quality", &baseline);
    print_recall_quality_report(&baseline);
    assert!(
        check_quality_gate(&baseline, true),
        "recall_quality quality gate failed for {}",
        baseline.dataset_name
    );

    c.bench_function("recall_quality/synthetic_quality_suite", |bench| {
        bench.iter_batched(
            || parse_eval_dataset(SYNTHETIC_QUALITY),
            |dataset| {
                let report = run_eval_dataset(dataset);
                black_box(report)
            },
            BatchSize::SmallInput,
        )
    });
}

fn recall_smoke(c: &mut Criterion) {
    let baseline = run_eval_dataset(parse_eval_dataset(SYNTHETIC_SMOKE));
    write_eval_report_artifact("recall_smoke", &baseline);
    print_recall_quality_report(&baseline);

    c.bench_function("recall_quality/synthetic_smoke_suite", |bench| {
        bench.iter_batched(
            || parse_eval_dataset(SYNTHETIC_SMOKE),
            |dataset| {
                let report = run_eval_dataset(dataset);
                black_box(report)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, recall_quality, recall_smoke);
criterion_main!(benches);

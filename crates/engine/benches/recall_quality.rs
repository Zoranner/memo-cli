use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

#[path = "support/quality.rs"]
mod quality_support;

use quality_support::{
    assert_synthetic_quality_gate, parse_eval_dataset, print_recall_quality_report,
    run_eval_dataset, write_eval_report_artifact, SYNTHETIC_QUALITY,
};

fn recall_quality(c: &mut Criterion) {
    let baseline = run_eval_dataset(parse_eval_dataset(SYNTHETIC_QUALITY));
    assert_synthetic_quality_gate(&baseline, true);
    write_eval_report_artifact("recall_quality", &baseline);
    print_recall_quality_report(&baseline);

    c.bench_function("recall_quality/synthetic_quality_suite", |bench| {
        bench.iter_batched(
            || parse_eval_dataset(SYNTHETIC_QUALITY),
            |dataset| {
                let report = run_eval_dataset(dataset);
                assert_synthetic_quality_gate(&report, false);
                black_box(report)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, recall_quality);
criterion_main!(benches);

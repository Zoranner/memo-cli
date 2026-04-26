use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

#[path = "support/quality.rs"]
mod quality_support;

use quality_support::{parse_eval_dataset, run_eval_dataset, SYNTHETIC_QUALITY};

fn recall_quality(c: &mut Criterion) {
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

criterion_group!(benches, recall_quality);
criterion_main!(benches);

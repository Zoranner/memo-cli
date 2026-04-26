use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use memo_engine::RecallRequest;

#[path = "support/latency.rs"]
mod latency_support;

use latency_support::{
    episode, open_engine, seed_alias_engine, seed_bm25_engine, seed_current_state_engine,
    seed_graph_engine, seed_vector_engine,
};

fn bench_remember(c: &mut Criterion) {
    c.bench_function("remember_manual_episode", |b| {
        b.iter_batched(
            open_engine,
            |(_temp, engine)| {
                let id = engine
                    .remember(episode("Benchmark note about warehouse drones."))
                    .expect("remember benchmark episode");
                black_box(id);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_recall(c: &mut Criterion) {
    c.bench_function("recall_exact_alias_fast", |b| {
        b.iter_batched(
            seed_alias_engine,
            |(_temp, alias_engine)| {
                let result = alias_engine
                    .recall(RecallRequest {
                        query: "Ally".to_string(),
                        limit: 5,
                        deep: false,
                    })
                    .expect("recall alias");
                black_box(result.results.len());
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("recall_bm25_fast", |b| {
        b.iter_batched(
            seed_bm25_engine,
            |(_temp, bm25_engine)| {
                let result = bm25_engine
                    .recall(RecallRequest {
                        query: "warehouse drones".to_string(),
                        limit: 5,
                        deep: false,
                    })
                    .expect("recall bm25");
                black_box(result.results.len());
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("recall_graph_expansion", |b| {
        b.iter_batched(
            seed_graph_engine,
            |(_temp, graph_engine)| {
                let result = graph_engine
                    .recall(RecallRequest {
                        query: "Alice".to_string(),
                        limit: 5,
                        deep: false,
                    })
                    .expect("recall graph");
                black_box(result.results.len());
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("recall_deep_current_state_after_dream", |b| {
        b.iter_batched(
            seed_current_state_engine,
            |(_temp, current_engine)| {
                let result = current_engine
                    .recall(RecallRequest {
                        query: "Where is Alice currently based?".to_string(),
                        limit: 5,
                        deep: true,
                    })
                    .expect("recall current state");
                black_box(result.results.len());
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("recall_vector_semantic_deterministic", |b| {
        b.iter_batched(
            seed_vector_engine,
            |(_temp, vector_engine)| {
                let result = vector_engine
                    .recall(RecallRequest {
                        query: "autonomous warehouse aircraft".to_string(),
                        limit: 5,
                        deep: true,
                    })
                    .expect("recall vector semantic");
                black_box(result.results.len());
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_remember, bench_recall);
criterion_main!(benches);

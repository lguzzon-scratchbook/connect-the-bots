use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use attractor_types::Context;
use std::collections::HashMap;

fn bench_context_set(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let context = Context::new();

    c.bench_function("context/set_single", |b| {
        b.to_async(&rt).iter(|| async {
            context.set("key", serde_json::json!("value")).await;
        })
    });
}

fn bench_context_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let context = Context::new();

    rt.block_on(async {
        for i in 0..100 {
            context.set(&format!("key{}", i), serde_json::json!(i)).await;
        }
    });

    c.bench_function("context/get_existing", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = context.get("key50").await;
        })
    });
}

fn bench_context_apply_updates(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let context = Context::new();

    let mut group = c.benchmark_group("context/apply_updates");
    for size in [10, 50, 100].iter() {
        let updates: HashMap<String, serde_json::Value> = (0..*size)
            .map(|i| (format!("key{}", i), serde_json::json!(i)))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &updates, |b, updates| {
            b.to_async(&rt).iter(|| async {
                context.apply_updates(updates.clone()).await;
            })
        });
    }
    group.finish();
}

fn bench_context_snapshot(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let context = Context::new();

    rt.block_on(async {
        for i in 0..100 {
            context.set(&format!("key{}", i), serde_json::json!(i)).await;
        }
    });

    c.bench_function("context/snapshot_100_keys", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = context.snapshot().await;
        })
    });
}

fn bench_context_concurrent_ops(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("context/concurrent_10_sets", |b| {
        b.to_async(&rt).iter(|| async {
            let ctx = Context::new();
            let mut handles = vec![];
            for i in 0..10 {
                let ctx = ctx.clone();
                handles.push(tokio::spawn(async move {
                    ctx.set(&format!("key{}", i), serde_json::json!(i)).await;
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    bench_context_set,
    bench_context_get,
    bench_context_apply_updates,
    bench_context_snapshot,
    bench_context_concurrent_ops
);
criterion_main!(benches);

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use std::thread;
use vector_db::arena::*;

fn allocation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Allocation");

    group.bench_function("single_thread", |b| {
        let arena = Arena::<usize>::new();
        b.iter(|| {
            for i in 0..1000 {
                black_box(arena.alloc(i));
            }
            arena.clear();
        })
    });

    group.bench_function("concurrent_4_threads", |b| {
        b.iter_custom(|iters| {
            let iters_per_thread = iters / 4;
            let start = std::time::Instant::now();
            let arena = Arc::new(Arena::new());

            let mut handles = Vec::new();
            for _ in 0..4 {
                let arena = Arc::clone(&arena);
                handles.push(thread::spawn(move || {
                    for _ in 0..iters_per_thread {
                        black_box(arena.alloc(0));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });
}

fn retrieval_benchmark(c: &mut Criterion) {
    let arena = Arena::new();
    let handles: Vec<Handle<usize>> = (0..10000).map(|i| arena.alloc(i)).collect();

    c.bench_function("Index retrieval", |b| {
        b.iter(|| {
            for handle in &handles {
                black_box(&arena[*handle]);
            }
        })
    });

    c.bench_function("Cloning retrieval", |b| {
        b.iter(|| {
            for handle in &handles {
                black_box(arena.get(*handle));
            }
        })
    });
}

criterion_group!(benches, allocation_benchmark, retrieval_benchmark);
criterion_main!(benches);

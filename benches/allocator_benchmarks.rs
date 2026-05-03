use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sim_mem_rs::memory::{NaiveAllocator, PagedAllocator, Allocator};

fn naive_allocator_benchmark(c: &mut Criterion) {
    c.bench_function("naive_allocator_allocate", |b| {
        b.iter(|| {
            let mut allocator = NaiveAllocator::new(1024);
            for i in 0..100 {
                let _ = black_box(allocator.allocate(10));
            }
        })
    });
}

fn paged_allocator_benchmark(c: &mut Criterion) {
    c.bench_function("paged_allocator_allocate", |b| {
        b.iter(|| {
            let mut allocator = PagedAllocator::new(1024, 64);
            for i in 0..100 {
                let _ = black_box(allocator.allocate(10));
            }
        })
    });
}

fn naive_allocator_deallocate_benchmark(c: &mut Criterion) {
    c.bench_function("naive_allocator_deallocate", |b| {
        b.iter(|| {
            let mut allocator = NaiveAllocator::new(1024);
            let mut blocks = Vec::new();
            for i in 0..100 {
                if let Some(block) = allocator.allocate(10) {
                    blocks.push(block);
                }
            }
            for block in blocks {
                black_box(allocator.deallocate(block.request_id));
            }
        })
    });
}

fn paged_allocator_deallocate_benchmark(c: &mut Criterion) {
    c.bench_function("paged_allocator_deallocate", |b| {
        b.iter(|| {
            let mut allocator = PagedAllocator::new(1024, 64);
            let mut blocks = Vec::new();
            for i in 0..100 {
                if let Some(block) = allocator.allocate(10) {
                    blocks.push(block);
                }
            }
            for block in blocks {
                black_box(allocator.deallocate(block.request_id));
            }
        })
    });
}

criterion_group!(
    benches,
    naive_allocator_benchmark,
    paged_allocator_benchmark,
    naive_allocator_deallocate_benchmark,
    paged_allocator_deallocate_benchmark
);
criterion_main!(benches);
#![allow(unused_must_use)]
#![allow(unused_variables)]
#![allow(dead_code)]

use criterion::*;
use queue_rs::queue::{QueueBehavior, QueueSize, QueueType};

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("offer");
  let mut q1 = queue_rs::queue::create_queue(QueueType::Vec, QueueSize::Limitless);
  let op = 1;
  group.bench_with_input(BenchmarkId::new("queue/vec", op), &op, |b, i| b.iter(|| q1.offer(*i)));
}
criterion_group!(benches, criterion_benchmark);

criterion_main! {
benches,
}

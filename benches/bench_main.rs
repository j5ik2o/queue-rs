#![allow(unused_must_use)]
#![allow(unused_variables)]
#![allow(dead_code)]

use criterion::*;
use queue_rs::queue::{QueueBehavior, QueueSize, QueueType};

fn offer(c: &mut Criterion) {
  let mut group = c.benchmark_group("offer");
  let mut vec_deque = queue_rs::queue::create_queue(QueueType::VecDequeue, QueueSize::Limitless);
  let mut queue_linked_list = queue_rs::queue::create_queue(QueueType::LinkedList, QueueSize::Limitless);
  let mut queue_mpsc = queue_rs::queue::create_queue(QueueType::MPSC, QueueSize::Limitless);
  let op = 1;
  group.bench_with_input(BenchmarkId::new("vec_deque", op), &op, |b, i| {
    b.iter(|| vec_deque.offer(*i))
  });
  group.bench_with_input(BenchmarkId::new("linked_list", op), &op, |b, i| {
    b.iter(|| queue_linked_list.offer(*i))
  });
  group.bench_with_input(BenchmarkId::new("queue_mpsc", op), &op, |b, i| {
    b.iter(|| queue_mpsc.offer(*i))
  });
}

fn poll(c: &mut Criterion) {
  let mut group = c.benchmark_group("poll");
  let mut values = [1; 10000];
  let mut vec_deque = queue_rs::queue::create_queue_with_elements(QueueType::VecDequeue, values.clone().into_iter());
  let mut queue_linked_list =
    queue_rs::queue::create_queue_with_elements(QueueType::LinkedList, values.clone().into_iter());
  let mut queue_mpsc = queue_rs::queue::create_queue_with_elements(QueueType::MPSC, values.clone().into_iter());
  let op = 1;
  group.bench_with_input(BenchmarkId::new("vec_deque", op), &op, |b, i| {
    b.iter(|| vec_deque.poll().unwrap())
  });
  group.bench_with_input(BenchmarkId::new("linked_list", op), &op, |b, i| {
    b.iter(|| queue_linked_list.poll().unwrap())
  });
  group.bench_with_input(BenchmarkId::new("queue_mpsc", op), &op, |b, i| {
    b.iter(|| queue_mpsc.poll().unwrap())
  });
}

criterion_group!(benches, offer, poll);

criterion_main! {
benches,
}

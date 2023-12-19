# queue-rs

This is a crate that provides queue related functions.

## usage

```rust
let queue_type = QueueType::VecDeque;
let size = QueueSize::Limited(32);

let mut queue = create_queue(queue_type, size);

queue.offer(1).unwrap();

let result: Option<i32> = queue.poll().unwrap();

let size = 10;
let mut blocking_queue = create_queue(queue_type, size).with_blocking();

queue.put(1).unwrap(); // If the queue is full, the current thread is blocked

let result: Option<i32> = queue.take().unwrap(); // If the queue is empty, the current thread is blocked
```

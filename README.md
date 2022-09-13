# queue-rs

This is a crate that provides queue related functions.

## usage

```rust
use queue_rs::create_queue;

let mut queue = create_queue(QueueType::Vec, Some(32));

queueq.offer(i).unwrap();

let result: Option<i32> = queue.poll().unwrap();
```
use std::cmp::Ordering;
use std::fmt::Debug;
use std::sync::Arc;

use anyhow::Result;
use thiserror::Error;

pub use blocking_queue::*;
pub use queue_mpsc::*;
pub use queue_vec::*;

mod blocking_queue;
mod queue_mpsc;
mod queue_vec;

pub trait Element: Debug + Clone + Send + Sync {}

impl Element for i8 {}

impl Element for i16 {}

impl Element for i32 {}

impl Element for i64 {}

impl Element for u8 {}

impl Element for u16 {}

impl Element for u32 {}

impl Element for u64 {}

impl Element for usize {}

impl Element for f32 {}

impl Element for f64 {}

impl Element for String {}

impl<T: Debug + Clone + Send + Sync> Element for Box<T> {}

impl<T: Debug + Clone + Send + Sync> Element for Arc<T> {}

#[derive(Error, Debug)]
pub enum QueueError<E> {
  #[error("Failed to offer an element: {0:?}")]
  OfferError(E),
  #[error("Failed to pool an element")]
  PoolError,
  #[error("Failed to peek an element")]
  PeekError,
  #[error("Failed to interrupt")]
  InterruptedError,
}

#[derive(Debug, Clone)]
pub enum QueueSize {
  Limitless,
  Limited(usize),
}

impl QueueSize {
  fn increment(&mut self) {
    match self {
      QueueSize::Limited(c) => {
        *c += 1;
      }
      _ => {}
    }
  }

  fn decrement(&mut self) {
    match self {
      QueueSize::Limited(c) => {
        *c -= 1;
      }
      _ => {}
    }
  }
}

impl PartialEq<Self> for QueueSize {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (QueueSize::Limitless, QueueSize::Limitless) => true,
      (QueueSize::Limited(l), QueueSize::Limited(r)) => l == r,
      _ => false,
    }
  }
}

impl PartialOrd<QueueSize> for QueueSize {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    match (self, other) {
      (QueueSize::Limitless, QueueSize::Limitless) => Some(Ordering::Equal),
      (QueueSize::Limitless, _) => Some(Ordering::Greater),
      (_, QueueSize::Limitless) => Some(Ordering::Less),
      (QueueSize::Limited(l), QueueSize::Limited(r)) => l.partial_cmp(r),
    }
  }
}

pub trait QueueBehavior<E>: Send + Sized {
  /// Returns whether this queue is empty.<br/>
  /// このキューが空かどうかを返します。
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Returns whether this queue is non-empty.<br/>
  /// このキューが空でないかどうかを返します。
  fn non_empty(&self) -> bool {
    !self.is_empty()
  }

  /// Returns whether the queue size has reached its capacity.<br/>
  /// このキューのサイズが容量まで到達したかどうかを返します。
  fn is_full(&self) -> bool {
    self.capacity() == self.len()
  }

  /// Returns whether the queue size has not reached its capacity.<br/>
  /// このキューのサイズが容量まで到達してないかどうかを返します。
  fn non_full(&self) -> bool {
    !self.is_full()
  }

  /// Returns the length of this queue.<br/>
  /// このキューの長さを返します。
  fn len(&self) -> QueueSize;

  /// Returns the capacity of this queue.<br/>
  /// このキューの最大容量を返します。
  fn capacity(&self) -> QueueSize;

  /// The specified element will be inserted into this queue,
  /// if the queue can be executed immediately without violating the capacity limit.<br/>
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された要素をこのキューに挿入します。
  fn offer(&mut self, e: E) -> Result<()>;

  /// Retrieves and deletes the head of the queue. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
  fn poll(&mut self) -> Result<Option<E>>;
}

pub trait HasPeekBehavior<E: Element>: QueueBehavior<E> {
  /// Gets the head of the queue, but does not delete it. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
  fn peek(&self) -> Result<Option<E>>;
}

pub trait BlockingQueueBehavior<E: Element>: QueueBehavior<E> + Send {
  /// Inserts the specified element into this queue. If necessary, waits until space is available.<br/>
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
  fn put(&mut self, e: E) -> Result<()>;

  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.<br/>
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  fn take(&mut self) -> Result<Option<E>>;

  /// Interrupts the operation of this queue.<br/>
  /// このキューの操作を中断します。
  fn interrupt(&mut self);

  /// Returns whether the operation of this queue has been interrupted.<br/>
  /// このキューの操作が中断されたかどうかを返します。
  fn is_interrupted(&self) -> bool;
}

pub enum QueueType {
  Vec,
  MPSC,
}

#[derive(Debug, Clone)]
pub enum Queue<T> {
  Vec(QueueVec<T>),
  MPSC(QueueMPSC<T>),
}

impl<T: Element + 'static> Queue<T> {
  pub fn with_blocking(self) -> BlockingQueue<T, Queue<T>> {
    BlockingQueue::new(self)
  }
}

impl<T: Element + 'static> QueueBehavior<T> for Queue<T> {
  fn len(&self) -> QueueSize {
    match self {
      Queue::Vec(inner) => inner.len(),
      Queue::MPSC(inner) => inner.len(),
    }
  }

  fn capacity(&self) -> QueueSize {
    match self {
      Queue::Vec(inner) => inner.capacity(),
      Queue::MPSC(inner) => inner.capacity(),
    }
  }

  fn offer(&mut self, e: T) -> Result<()> {
    match self {
      Queue::Vec(inner) => inner.offer(e),
      Queue::MPSC(inner) => inner.offer(e),
    }
  }

  fn poll(&mut self) -> Result<Option<T>> {
    match self {
      Queue::Vec(inner) => inner.poll(),
      Queue::MPSC(inner) => inner.poll(),
    }
  }
}

pub fn create_queue<T: Element + 'static>(queue_type: QueueType, num_elements: Option<usize>) -> Queue<T> {
  match (queue_type, num_elements) {
    (QueueType::Vec, None) => Queue::Vec(QueueVec::<T>::new()),
    (QueueType::Vec, Some(num)) => Queue::Vec(QueueVec::<T>::with_num_elements(num)),
    (QueueType::MPSC, None) => Queue::MPSC(QueueMPSC::<T>::new()),
    (QueueType::MPSC, Some(num)) => Queue::MPSC(QueueMPSC::<T>::with_num_elements(num)),
  }
}

#[cfg(test)]
mod tests {
  use serial_test::serial;
  use std::sync::{Arc, Condvar, Mutex};
  use std::{env, thread};

  use crate::queue::{create_queue, QueueBehavior, QueueType};
  use crate::queue::{BlockingQueueBehavior, QueueSize};
  extern crate env_logger;

  fn init_logger() {
    env::set_var("RUST_LOG", "debug");
    // env::set_var("RUST_LOG", "trace");
    let _ = env_logger::try_init();
  }

  struct CountDownLatch {
    count: Mutex<usize>,
    condvar: Condvar,
  }

  impl CountDownLatch {
    fn new(count: usize) -> Self {
      CountDownLatch {
        count: Mutex::new(count),
        condvar: Condvar::new(),
      }
    }

    fn count_down(&self) {
      let mut count = self.count.lock().unwrap();
      *count -= 1;
      if *count == 0 {
        self.condvar.notify_all();
      }
    }

    fn wait(&self) {
      let mut count = self.count.lock().unwrap();
      while *count > 0 {
        count = self.condvar.wait(count).unwrap();
      }
    }
  }

  #[test]
  #[serial]
  fn test_blocking_put() {
    init_logger();
    let size: usize = 10;
    let mut bqv1 = create_queue(QueueType::Vec, Some(size)).with_blocking();
    let mut bqv2 = bqv1.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..size {
        log::debug!("take: start: {}", i);
        let n = bqv2.put(i as i32).unwrap();
        log::debug!("take: finish: {},{:?}", i, n);
      }
      assert_eq!(bqv2.len(), QueueSize::Limited(size));

      bqv2.interrupt();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(e) => {
          log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());

      please_interrupt_cloned.count_down();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(e) => {
          log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());
    });

    please_interrupt.wait();
    bqv1.interrupt();
    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  fn test_put_with_take() {
    init_logger();
    let capacity = 2;

    let mut bqv1 = create_queue(QueueType::Vec, Some(capacity)).with_blocking();
    let mut bqv2 = bqv1.clone();

    let please_take = Arc::new(CountDownLatch::new(1));
    let please_take_cloned = please_take.clone();
    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..capacity {
        log::debug!("take: start: {}", i);
        let n = bqv2.put(i);
        log::debug!("take: finish: {},{:?}", i, n);
      }
      please_take_cloned.count_down();
      bqv2.put(86).unwrap();

      please_interrupt_cloned.count_down();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(e) => {
          log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
    });

    please_take.wait();

    let r = bqv1.take().unwrap();
    assert_eq!(r, Some(0));

    please_interrupt.wait();
    bqv1.interrupt();

    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  pub fn test_blocking_take() {
    init_logger();
    let size: usize = 10;
    let mut bqv1 = create_queue::<i32>(QueueType::Vec, Some(size)).with_blocking();
    for i in 0..size {
      bqv1.offer(i as i32).unwrap();
    }

    let mut bqv2 = bqv1.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..size {
        log::debug!("take: start: {}", i);
        let n = bqv2.take().unwrap();
        log::debug!("take: finish: {},{:?}", i, n);
      }

      bqv2.interrupt();
      match bqv2.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(e) => {
          log::debug!("take: finish: error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());

      please_interrupt_cloned.count_down();
      match bqv2.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(e) => {
          log::debug!("take: finish: error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());
    });

    please_interrupt.wait();
    bqv1.interrupt();
    handler1.join().unwrap();
  }
}

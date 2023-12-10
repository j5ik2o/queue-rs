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
  #[error("Failed to contains an element")]
  ContainsError,
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

  pub fn is_limitless(&self) -> bool {
    match self {
      QueueSize::Limitless => true,
      _ => false,
    }
  }

  pub fn to_option(&self) -> Option<usize> {
    match self {
      QueueSize::Limitless => None,
      QueueSize::Limited(c) => Some(*c),
    }
  }

  pub fn to_usize(&self) -> usize {
    match self {
      QueueSize::Limitless => usize::MAX,
      QueueSize::Limited(c) => *c,
    }
  }

  pub fn get_or_else<F>(&self, default: F) -> usize
  where
    F: FnOnce() -> usize, {
    match self {
      QueueSize::Limitless => default(),
      QueueSize::Limited(c) => *c,
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

pub trait HasContainsBehavior<E: Element>: QueueBehavior<E> {
  fn contains(&self, element: &E) -> Result<bool>;
}

pub trait BlockingQueueBehavior<E: Element>: QueueBehavior<E> + Send {
  /// Inserts the specified element into this queue. If necessary, waits until space is available.<br/>
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
  fn put(&mut self, e: E) -> Result<()>;

  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.<br/>
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  fn take(&mut self) -> Result<Option<E>>;

  fn remaining_capacity(&self) -> QueueSize;

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

impl<E: Element + 'static> HasPeekBehavior<E> for Queue<E> {
  fn peek(&self) -> Result<Option<E>> {
    match self {
      Queue::Vec(inner) => inner.peek(),
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for Queue<E> {
  fn contains(&self, element: &E) -> Result<bool> {
    match self {
      Queue::Vec(inner) => inner.contains(element),
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

pub fn create_queue<T: Element + 'static>(queue_type: QueueType, num_elements: QueueSize) -> Queue<T> {
  match (queue_type, num_elements) {
    (QueueType::Vec, QueueSize::Limitless) => Queue::Vec(QueueVec::<T>::new()),
    (QueueType::Vec, QueueSize::Limited(num)) => Queue::Vec(QueueVec::<T>::with_num_elements(num)),
    (QueueType::MPSC, QueueSize::Limitless) => Queue::MPSC(QueueMPSC::<T>::new()),
    (QueueType::MPSC, QueueSize::Limited(num)) => Queue::MPSC(QueueMPSC::<T>::with_num_elements(num)),
  }
}

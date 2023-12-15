use std::cmp::Ordering;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use thiserror::Error;

use crate::queue::queue_linkedlist::QueueLinkedList;
pub use blocking_queue::*;
pub use queue_mpsc::*;
pub use queue_vec::*;

mod blocking_queue;
mod blocking_queue_test;
mod queue_linkedlist;
mod queue_mpsc;
mod queue_vec;

/// A trait that represents an element.<br/>
/// 要素を表すトレイト。
pub trait Element: Debug + Clone + Send + Sync {
  fn is_empty(&self) -> bool {
    false
  }
}

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

impl Element for String {
  fn is_empty(&self) -> bool {
    String::is_empty(self)
  }
}

impl<T: Debug + Clone + Send + Sync> Element for Box<T> {}

impl<T: Debug + Clone + Send + Sync> Element for Arc<T> {}

impl<T: Debug + Clone + Send + Sync> Element for Option<T> {
  fn is_empty(&self) -> bool {
    self.is_none()
  }
}

/// An error that occurs when a queue operation fails.<br/>
/// キューの操作に失敗した場合に発生するエラー。
#[derive(Error, Debug, PartialEq)]
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
  #[error("Failed to timeout")]
  TimeoutError,
}

/// The size of the queue.<br/>
/// キューのサイズ。
#[derive(Debug, Clone)]
pub enum QueueSize {
  /// The queue has no capacity limit.<br/>
  /// キューに容量制限がない。
  Limitless,
  /// The queue has a capacity limit.<br/>
  /// キューに容量制限がある。
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

  /// Returns whether the queue has no capacity limit.<br/>
  /// キューに容量制限がないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `false` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn is_limitless(&self) -> bool {
    match self {
      QueueSize::Limitless => true,
      _ => false,
    }
  }

  /// Converts to an option type.<br/>
  /// オプション型に変換します。
  ///
  /// # Return Value / 戻り値
  /// - `None` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `Some(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn to_option(&self) -> Option<usize> {
    match self {
      QueueSize::Limitless => None,
      QueueSize::Limited(c) => Some(*c),
    }
  }

  /// Converts to a usize type.<br/>
  /// usize型に変換します。
  ///
  /// # Return Value / 戻り値
  /// - `usize::MAX` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `num` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn to_usize(&self) -> usize {
    match self {
      QueueSize::Limitless => usize::MAX,
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

/// A trait that defines the behavior of a queue.<br/>
/// キューの振る舞いを定義するトレイト。
pub trait QueueBehavior<E>: Send + Sized {
  /// Returns whether this queue is empty.<br/>
  /// このキューが空かどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is empty. / キューが空の場合。
  /// - `false` - If the queue is not empty. / キューが空でない場合。
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Returns whether this queue is non-empty.<br/>
  /// このキューが空でないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is not empty. / キューが空でない場合。
  /// - `false` - If the queue is empty. / キューが空の場合。
  fn non_empty(&self) -> bool {
    !self.is_empty()
  }

  /// Returns whether the queue size has reached its capacity.<br/>
  /// このキューのサイズが容量まで到達したかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  /// - `false` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  fn is_full(&self) -> bool {
    self.capacity() == self.len()
  }

  /// Returns whether the queue size has not reached its capacity.<br/>
  /// このキューのサイズが容量まで到達してないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  /// - `false` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  fn non_full(&self) -> bool {
    !self.is_full()
  }

  /// Returns the length of this queue.<br/>
  /// このキューの長さを返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  fn len(&self) -> QueueSize;

  /// Returns the capacity of this queue.<br/>
  /// このキューの最大容量を返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  fn capacity(&self) -> QueueSize;

  /// The specified element will be inserted into this queue,
  /// if the queue can be executed immediately without violating the capacity limit.<br/>
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された要素をこのキューに挿入します。
  ///
  /// # Arguments / 引数
  /// - `element` - The element to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the element is inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the element cannot be inserted. / 要素を挿入できなかった場合。
  fn offer(&mut self, element: E) -> Result<()>;

  /// The specified elements will be inserted into this queue,
  /// if the queue can be executed immediately without violating the capacity limit.<br/>
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された複数の要素をこのキューに挿入します。
  ///
  /// # Arguments / 引数
  /// - `elements` - The elements to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the elements are inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the elements cannot be inserted. / 要素を挿入できなかった場合。
  fn offer_all(&mut self, elements: impl IntoIterator<Item = E>) -> Result<()> {
    for e in elements {
      self.offer(e)?;
    }
    Ok(())
  }

  /// Retrieves and deletes the head of the queue. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  fn poll(&mut self) -> Result<Option<E>>;
}

/// A trait that defines the behavior of a queue that can be peeked.<br/>
/// Peekができるキューの振る舞いを定義するトレイト。
pub trait HasPeekBehavior<E: Element>: QueueBehavior<E> {
  /// Gets the head of the queue, but does not delete it. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  fn peek(&self) -> Result<Option<E>>;
}

/// A trait that defines the behavior of a queue that can be checked for contains.<br/>
/// Containsができるキューの振る舞いを定義するトレイト。
pub trait HasContainsBehavior<E: Element>: QueueBehavior<E> {
  /// Returns whether the specified element is contained in this queue.<br/>
  /// 指定された要素がこのキューに含まれているかどうかを返します。
  ///
  /// # Arguments / 引数
  /// - `element` - The element to be checked. / チェックする要素。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the element is contained in this queue. / 要素がこのキューに含まれている場合。
  /// - `false` - If the element is not contained in this queue. / 要素がこのキューに含まれていない場合。
  fn contains(&self, element: &E) -> bool;
}

pub trait BlockingQueueBehavior<E: Element>: QueueBehavior<E> + Send {
  /// Inserts the specified element into this queue. If necessary, waits until space is available.<br/>
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
  ///
  /// # Arguments / 引数
  /// - `element` - The element to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the element is inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the element cannot be inserted. / 要素を挿入できなかった場合。
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted. / 操作が中断された場合。
  fn put(&mut self, element: E) -> Result<()>;

  /// Inserts the specified element into this queue. If necessary, waits until space is available. You can specify the waiting time.<br/>
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。待機時間を指定できます。
  ///
  /// # Arguments / 引数
  /// - `element` - The element to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the element is inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the element cannot be inserted. / 要素を挿入できなかった場合。
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted. / 操作が中断された場合。
  /// - `Err(QueueError::TimeoutError)` - If the operation times out. / 操作がタイムアウトした場合。
  fn put_timeout(&mut self, element: E, timeout: Duration) -> Result<()>;

  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.<br/>
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted. / 操作が中断された場合。
  fn take(&mut self) -> Result<Option<E>>;

  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available. You can specify the waiting time.<br/>
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。待機時間を指定できます。
  ///
  /// # Arguments / 引数
  /// - `timeout` - The maximum time to wait. / 待機する最大時間。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted. / 操作が中断された場合。
  /// - `Err(QueueError::TimeoutError)` - If the operation times out. / 操作がタイムアウトした場合。
  fn take_timeout(&mut self, timeout: Duration) -> Result<Option<E>>;

  /// Returns the number of elements that can be inserted into this queue without blocking.<br/>
  /// ブロックせずにこのキューに挿入できる要素数を返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  fn remaining_capacity(&self) -> QueueSize;

  /// Interrupts the operation of this queue.<br/>
  /// このキューの操作を中断します。
  fn interrupt(&mut self);

  /// Returns whether the operation of this queue has been interrupted.<br/>
  /// このキューの操作が中断されたかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the operation is interrupted. / 操作が中断された場合。
  /// - `false` - If the operation is not interrupted. / 操作が中断されていない場合。
  fn is_interrupted(&self) -> bool;
}

/// An enumeration type that represents the type of queue.<br/>
/// キューの種類を表す列挙型。
pub enum QueueType {
  /// A queue implemented with a vector.<br/>
  /// ベクタで実装されたキュー。
  Vec,
  /// A queue implemented with LinkedList.<br/>
  /// LinkedListで実装されたキュー。
  LinkedList,
  /// A queue implemented with MPSC.<br/>
  /// MPSCで実装されたキュー。
  MPSC,
}

/// An enumeration type that represents a queue.<br/>
/// キューを表す列挙型。
#[derive(Debug, Clone)]
pub enum Queue<T> {
  /// A queue implemented with a vector.<br/>
  /// ベクタで実装されたキュー。
  Vec(QueueVec<T>),
  /// A queue implemented with LinkedList.<br/>
  /// LinkedListで実装されたキュー。
  LinkedList(QueueLinkedList<T>),
  /// A queue implemented with MPSC.<br/>
  /// MPSCで実装されたキュー。
  MPSC(QueueMPSC<T>),
}

impl<T: Element + 'static> Queue<T> {
  /// Converts the queue to a blocking queue.<br/>
  /// キューをブロッキングキューに変換します。
  ///
  /// # Return Value / 戻り値
  /// - `BlockingQueue<T, Queue<T>>` - A blocking queue. / ブロッキングキュー。
  pub fn with_blocking(self) -> BlockingQueue<T, Queue<T>> {
    BlockingQueue::new(self)
  }

  pub fn iter(&mut self) -> QueueIter<T, Queue<T>> {
    QueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

impl<T: Element + 'static> QueueBehavior<T> for Queue<T> {
  fn len(&self) -> QueueSize {
    match self {
      Queue::Vec(inner) => inner.len(),
      Queue::LinkedList(inner) => inner.len(),
      Queue::MPSC(inner) => inner.len(),
    }
  }

  fn capacity(&self) -> QueueSize {
    match self {
      Queue::Vec(inner) => inner.capacity(),
      Queue::LinkedList(inner) => inner.capacity(),
      Queue::MPSC(inner) => inner.capacity(),
    }
  }

  fn offer(&mut self, element: T) -> Result<()> {
    match self {
      Queue::Vec(inner) => inner.offer(element),
      Queue::LinkedList(inner) => inner.offer(element),
      Queue::MPSC(inner) => inner.offer(element),
    }
  }

  fn offer_all(&mut self, elements: impl IntoIterator<Item = T>) -> Result<()> {
    match self {
      Queue::Vec(inner) => inner.offer_all(elements),
      Queue::LinkedList(inner) => inner.offer_all(elements),
      Queue::MPSC(inner) => inner.offer_all(elements),
    }
  }

  fn poll(&mut self) -> Result<Option<T>> {
    match self {
      Queue::Vec(inner) => inner.poll(),
      Queue::LinkedList(inner) => inner.poll(),
      Queue::MPSC(inner) => inner.poll(),
    }
  }
}

impl<E: Element + 'static> HasPeekBehavior<E> for Queue<E> {
  fn peek(&self) -> Result<Option<E>> {
    match self {
      Queue::Vec(inner) => inner.peek(),
      Queue::LinkedList(inner) => inner.peek(),
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for Queue<E> {
  fn contains(&self, element: &E) -> bool {
    match self {
      Queue::Vec(inner) => inner.contains(element),
      Queue::LinkedList(inner) => inner.contains(element),
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

impl<E: Element + 'static> IntoIterator for Queue<E> {
  type IntoIter = QueueIntoIter<E, Queue<E>>;
  type Item = E;

  fn into_iter(self) -> Self::IntoIter {
    QueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

pub struct QueueIntoIter<E, Q> {
  q: Q,
  p: PhantomData<E>,
}

impl<E, Q: QueueBehavior<E>> Iterator for QueueIntoIter<E, Q> {
  type Item = E;

  fn next(&mut self) -> Option<Self::Item> {
    self.q.poll().unwrap()
  }
}

pub struct QueueIter<'a, E, Q> {
  q: &'a mut Q,
  p: PhantomData<E>,
}

impl<'a, E, Q: QueueBehavior<E>> Iterator for QueueIter<'a, E, Q> {
  type Item = E;

  fn next(&mut self) -> Option<Self::Item> {
    self.q.poll().unwrap()
  }
}

pub fn create_queue<T: Element + 'static>(queue_type: QueueType, num_elements: QueueSize) -> Queue<T> {
  match (queue_type, num_elements) {
    (QueueType::Vec, QueueSize::Limitless) => Queue::Vec(QueueVec::<T>::new()),
    (QueueType::Vec, QueueSize::Limited(num)) => Queue::Vec(QueueVec::<T>::new().with_num_elements(num)),
    (QueueType::LinkedList, QueueSize::Limitless) => Queue::LinkedList(QueueLinkedList::<T>::new()),
    (QueueType::LinkedList, QueueSize::Limited(num)) => {
      Queue::LinkedList(QueueLinkedList::<T>::new().with_num_elements(num))
    }
    (QueueType::MPSC, QueueSize::Limitless) => Queue::MPSC(QueueMPSC::<T>::new()),
    (QueueType::MPSC, QueueSize::Limited(num)) => Queue::MPSC(QueueMPSC::<T>::new().with_num_elements(num)),
  }
}

pub fn create_queue_with_elements<T: Element + 'static>(
  queue_type: QueueType,
  values: impl IntoIterator<Item = T> + ExactSizeIterator,
) -> Queue<T> {
  match queue_type {
    QueueType::Vec => Queue::Vec(QueueVec::<T>::new().with_elements(values)),
    QueueType::LinkedList => Queue::LinkedList(QueueLinkedList::<T>::new().with_elements(values)),
    QueueType::MPSC => Queue::MPSC(QueueMPSC::<T>::new().with_elements(values)),
  }
}

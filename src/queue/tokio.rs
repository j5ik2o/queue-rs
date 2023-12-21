use futures::Stream;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::queue::tokio::blocking_queue::BlockingQueue;
use crate::queue::tokio::queue_linkedlist::QueueLinkedList;
use crate::queue::tokio::queue_mpsc::QueueMPSC;
use crate::queue::tokio::queue_vec::QueueVec;
use crate::queue::{Element, QueueSize, QueueType};

mod blocking_queue;
mod blocking_queue_test;
mod queue_linkedlist;
mod queue_mpsc;
mod queue_vec;

/// A trait that defines the behavior of a queue.<br/>
/// キューの振る舞いを定義するトレイト。
#[async_trait::async_trait]
pub trait QueueBehavior<E: Element>: Send + Sized + Sync {
  /// Returns whether this queue is empty.<br/>
  /// このキューが空かどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is empty. / キューが空の場合。
  /// - `false` - If the queue is not empty. / キューが空でない場合。
  async fn is_empty(&self) -> bool {
    self.len().await == QueueSize::Limited(0)
  }

  /// Returns whether this queue is non-empty.<br/>
  /// このキューが空でないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is not empty. / キューが空でない場合。
  /// - `false` - If the queue is empty. / キューが空の場合。
  async fn non_empty(&self) -> bool {
    !self.is_empty().await
  }

  /// Returns whether the queue size has reached its capacity.<br/>
  /// このキューのサイズが容量まで到達したかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  /// - `false` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  async fn is_full(&self) -> bool {
    self.capacity().await == self.len().await
  }

  /// Returns whether the queue size has not reached its capacity.<br/>
  /// このキューのサイズが容量まで到達してないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  /// - `false` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  async fn non_full(&self) -> bool {
    !self.is_full().await
  }

  /// Returns the length of this queue.<br/>
  /// このキューの長さを返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  async fn len(&self) -> QueueSize;

  /// Returns the capacity of this queue.<br/>
  /// このキューの最大容量を返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  async fn capacity(&self) -> QueueSize;

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
  async fn offer(&mut self, element: E) -> anyhow::Result<()>;

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
  async fn offer_all(&mut self, elements: impl IntoIterator<Item = E> + Send) -> anyhow::Result<()> {
    let elements: Vec<E> = elements.into_iter().collect();
    for e in elements {
      self.offer(e).await?;
    }
    Ok(())
  }

  /// Retrieves and deletes the head of the queue. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  async fn poll(&mut self) -> anyhow::Result<Option<E>>;
}

/// A trait that defines the behavior of a queue that can be peeked.<br/>
/// Peekができるキューの振る舞いを定義するトレイト。
#[async_trait::async_trait]
pub trait HasPeekBehavior<E: Element>: QueueBehavior<E> {
  /// Gets the head of the queue, but does not delete it. Returns None if the queue is empty.<br/>
  /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  async fn peek(&self) -> anyhow::Result<Option<E>>;
}

/// A trait that defines the behavior of a queue that can be checked for contains.<br/>
/// Containsができるキューの振る舞いを定義するトレイト。
#[async_trait::async_trait]
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
  async fn contains(&self, element: &E) -> bool;
}

#[async_trait::async_trait]
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
  async fn put(&mut self, element: E) -> anyhow::Result<()>;

  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.<br/>
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(Some(element))` - If the element is retrieved successfully. / 要素が正常に取得された場合。
  /// - `Ok(None)` - If the queue is empty. / キューが空の場合。
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted. / 操作が中断された場合。
  async fn take(&mut self) -> anyhow::Result<Option<E>>;

  /// Returns the number of elements that can be inserted into this queue without blocking.<br/>
  /// ブロックせずにこのキューに挿入できる要素数を返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  async fn remaining_capacity(&self) -> QueueSize;

  /// Interrupts the operation of this queue.<br/>
  /// このキューの操作を中断します。
  async fn interrupt(&mut self);

  /// Returns whether the operation of this queue has been interrupted.<br/>
  /// このキューの操作が中断されたかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the operation is interrupted. / 操作が中断された場合。
  /// - `false` - If the operation is not interrupted. / 操作が中断されていない場合。
  async fn is_interrupted(&self) -> bool;
}

/// An enumeration type that represents a queue.<br/>
/// キューを表す列挙型。
#[derive(Debug, Clone)]
pub enum Queue<T> {
  /// A queue implemented with a vector.<br/>
  /// ベクタで実装されたキュー。
  VecDequeue(QueueVec<T>),
  /// A queue implemented with LinkedList.<br/>
  /// LinkedListで実装されたキュー。
  LinkedList(QueueLinkedList<T>),
  /// A queue implemented with MPSC.<br/>
  /// MPSCで実装されたキュー。
  MPSC(QueueMPSC<T>),
}

impl<T: Element + 'static> Queue<T> {
  pub fn as_vec_dequeue_mut(&mut self) -> Option<&mut QueueVec<T>> {
    match self {
      Queue::VecDequeue(inner) => Some(inner),
      _ => None,
    }
  }

  pub fn as_vec_dequeue(&self) -> Option<&QueueVec<T>> {
    match self {
      Queue::VecDequeue(inner) => Some(inner),
      _ => None,
    }
  }

  pub fn as_linked_list_mut(&mut self) -> Option<&mut QueueLinkedList<T>> {
    match self {
      Queue::LinkedList(inner) => Some(inner),
      _ => None,
    }
  }

  pub fn as_linked_list(&self) -> Option<&QueueLinkedList<T>> {
    match self {
      Queue::LinkedList(inner) => Some(inner),
      _ => None,
    }
  }

  pub fn as_mpsc_mut(&mut self) -> Option<&mut QueueMPSC<T>> {
    match self {
      Queue::MPSC(inner) => Some(inner),
      _ => None,
    }
  }

  pub fn as_mpsc(&self) -> Option<&QueueMPSC<T>> {
    match self {
      Queue::MPSC(inner) => Some(inner),
      _ => None,
    }
  }

  /// Converts the queue to a blocking queue.<br/>
  /// キューをブロッキングキューに変換します。
  ///
  /// # Return Value / 戻り値
  /// - `BlockingQueue<T, Queue<T>>` - A blocking queue. / ブロッキングキュー。
  pub fn with_blocking(self) -> BlockingQueue<T, Queue<T>> {
    BlockingQueue::new(self)
  }

  /// Gets an iterator for this queue.
  /// このキューのイテレータを取得します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueIter<T, Queue<T>>` - An iterator for this queue. / このキューのイテレータ。
  pub fn iter(&mut self) -> crate::queue::QueueIter<T, Queue<T>> {
    crate::queue::QueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

#[async_trait::async_trait]
impl<T: Element + 'static> QueueBehavior<T> for Queue<T> {
  async fn len(&self) -> QueueSize {
    match self {
      Queue::VecDequeue(inner) => inner.len().await,
      Queue::LinkedList(inner) => inner.len().await,
      Queue::MPSC(inner) => inner.len().await,
    }
  }

  async fn capacity(&self) -> QueueSize {
    match self {
      Queue::VecDequeue(inner) => inner.capacity().await,
      Queue::LinkedList(inner) => inner.capacity().await,
      Queue::MPSC(inner) => inner.capacity().await,
    }
  }

  async fn offer(&mut self, element: T) -> anyhow::Result<()> {
    match self {
      Queue::VecDequeue(inner) => inner.offer(element).await,
      Queue::LinkedList(inner) => inner.offer(element).await,
      Queue::MPSC(inner) => inner.offer(element).await,
    }
  }

  async fn offer_all(&mut self, elements: impl IntoIterator<Item = T> + Send) -> anyhow::Result<()> {
    match self {
      Queue::VecDequeue(inner) => inner.offer_all(elements).await,
      Queue::LinkedList(inner) => inner.offer_all(elements).await,
      Queue::MPSC(inner) => inner.offer_all(elements).await,
    }
  }

  async fn poll(&mut self) -> anyhow::Result<Option<T>> {
    match self {
      Queue::VecDequeue(inner) => inner.poll().await,
      Queue::LinkedList(inner) => inner.poll().await,
      Queue::MPSC(inner) => inner.poll().await,
    }
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static> HasPeekBehavior<E> for Queue<E> {
  async fn peek(&self) -> anyhow::Result<Option<E>> {
    match self {
      Queue::VecDequeue(inner) => inner.peek().await,
      Queue::LinkedList(inner) => inner.peek().await,
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

#[async_trait::async_trait]
impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for Queue<E> {
  async fn contains(&self, element: &E) -> bool {
    match self {
      Queue::VecDequeue(inner) => inner.contains(element).await,
      Queue::LinkedList(inner) => inner.contains(element).await,
      Queue::MPSC(_) => panic!("Not supported implementation."),
    }
  }
}

pub struct QueueIter<E, Q> {
  q: Q,
  current_future: Option<Pin<Box<dyn Future<Output = Result<Option<E>, anyhow::Error>> + Send>>>,
  p: PhantomData<E>,
}

impl<E: Element + Unpin + 'static, Q: QueueBehavior<E> + Unpin> Stream for QueueIter<E, Q> {
  type Item = E;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let bq = &mut self.get_mut().q;
    match Pin::new(&mut bq.poll()).poll(cx) {
      Poll::Ready(Ok(Some(value))) => Poll::Ready(Some(value)),
      Poll::Ready(Ok(None)) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
      _ => {
        panic!("BlockingQueueIter: poll_next: unexpected error");
      }
    }
  }
}

pub async fn create_queue<T: Element + 'static>(queue_type: QueueType, queue_size: QueueSize) -> Queue<T> {
  match (queue_type, &queue_size) {
    (QueueType::VecDequeue, QueueSize::Limitless) => Queue::VecDequeue(QueueVec::<T>::new()),
    (QueueType::VecDequeue, QueueSize::Limited(_)) => Queue::VecDequeue(QueueVec::<T>::new().with_capacity(queue_size)),
    (QueueType::LinkedList, QueueSize::Limitless) => Queue::LinkedList(QueueLinkedList::<T>::new()),
    (QueueType::LinkedList, QueueSize::Limited(_)) => {
      Queue::LinkedList(QueueLinkedList::<T>::new().with_capacity(queue_size))
    }
    (QueueType::MPSC, QueueSize::Limitless) => Queue::MPSC(QueueMPSC::<T>::new(queue_size.to_usize())),
    (QueueType::MPSC, QueueSize::Limited(_)) => Queue::MPSC(
      QueueMPSC::<T>::new(queue_size.to_usize())
        .with_capacity(queue_size)
        .await,
    ),
  }
}

pub async fn create_queue_with_elements<T: Element + 'static>(
  queue_type: QueueType,
  queue_size: QueueSize,
  values: impl IntoIterator<Item = T> + Send,
) -> Queue<T> {
  let vec = values.into_iter().collect::<Vec<_>>();
  match queue_type {
    QueueType::VecDequeue => Queue::VecDequeue(QueueVec::<T>::new().with_capacity(queue_size).with_elements(vec)),
    QueueType::LinkedList => {
      Queue::LinkedList(QueueLinkedList::<T>::new().with_capacity(queue_size).with_elements(vec))
    }
    QueueType::MPSC => Queue::MPSC(
      QueueMPSC::<T>::new(vec.len())
        .with_capacity(queue_size)
        .await
        .with_elements(vec)
        .await,
    ),
  }
}

mod queue_linkedlist;
mod queue_mpsc;
mod queue_vec;

use crate::queue::{Element, Queue, QueueSize};
use std::marker::PhantomData;

/// A trait that defines the behavior of a queue.<br/>
/// キューの振る舞いを定義するトレイト。
#[async_trait::async_trait]
pub trait QueueBehavior<E>: Send + Sized {
  /// Returns whether this queue is empty.<br/>
  /// このキューが空かどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is empty. / キューが空の場合。
  /// - `false` - If the queue is not empty. / キューが空でない場合。
  async fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Returns whether this queue is non-empty.<br/>
  /// このキューが空でないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is not empty. / キューが空でない場合。
  /// - `false` - If the queue is empty. / キューが空の場合。
  async fn non_empty(&self) -> bool {
    !self.is_empty()
  }

  /// Returns whether the queue size has reached its capacity.<br/>
  /// このキューのサイズが容量まで到達したかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  /// - `false` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  async fn is_full(&self) -> bool {
    self.capacity() == self.len()
  }

  /// Returns whether the queue size has not reached its capacity.<br/>
  /// このキューのサイズが容量まで到達してないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  /// - `false` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  async fn non_full(&self) -> bool {
    !self.is_full()
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
  async fn offer_all(&mut self, elements: impl IntoIterator<Item = E>) -> anyhow::Result<()> {
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
  async fn poll(&mut self) -> anyhow::Result<Option<E>>;
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
  async fn peek(&self) -> anyhow::Result<Option<E>>;
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
  async fn contains(&self, element: &E) -> bool;
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
    self.q.poll().ok().flatten()
  }
}
impl<E: Element + 'static, Q: QueueBehavior<E>> ExactSizeIterator for QueueIntoIter<E, Q> {
  fn len(&self) -> usize {
    self.q.len().to_usize()
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

impl<'a, E: Element + 'static, Q: QueueBehavior<E>> ExactSizeIterator for QueueIter<'a, E, Q> {
  fn len(&self) -> usize {
    self.q.len().to_usize()
  }
}

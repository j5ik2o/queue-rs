use crate::queue::tokio::{HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueIntoIter};
use crate::queue::{Element, QueueError, QueueIter, QueueSize};
use std::collections::LinkedList;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A queue implementation backed by a `LinkedList`.<br/>
/// `LinkedList` で実装されたキュー。
#[derive(Debug, Clone)]
pub struct QueueLinkedList<E> {
  elements: Arc<Mutex<LinkedList<E>>>,
  capacity: QueueSize,
}

impl<E: Element + 'static> QueueBehavior<E> for QueueLinkedList<E> {
  async fn len(&self) -> QueueSize {
    let mg = self.elements.lock().await;
    let len = mg.len();
    QueueSize::Limited(len)
  }

  async fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  async fn offer(&mut self, element: E) -> anyhow::Result<()> {
    if self.non_full() {
      let mut mg = self.elements.lock().await;
      mg.push_back(element);
      Ok(())
    } else {
      Err(QueueError::OfferError(element).into())
    }
  }

  async fn poll(&mut self) -> anyhow::Result<Option<E>> {
    let mut mg = self.elements.lock().await;
    Ok(mg.pop_front())
  }
}

impl<E: Element + 'static> QueueLinkedList<E> {
  /// Create a new `QueueLinkedList`.<br/>
  /// 新しい `QueueLinkedList` を作成します。
  pub fn new() -> Self {
    Self {
      elements: Arc::new(Mutex::new(LinkedList::new())),
      capacity: QueueSize::Limitless,
    }
  }

  /// Update the maximum number of elements in the queue.<br/>
  /// キューの最大要素数を更新します。
  ///
  /// # Arguments / 引数
  /// - `capacity` - The maximum number of elements in the queue. / キューの最大要素数。
  pub fn with_capacity(mut self, capacity: QueueSize) -> Self {
    self.capacity = capacity;
    self
  }

  /// Update the elements in the queue.<br/>
  /// キューの要素を更新します。
  ///
  /// # Arguments / 引数
  /// - `elements` - The elements to be updated.<br/>
  pub fn with_elements(mut self, elements: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let vec = elements.into_iter().collect::<LinkedList<E>>();
    self.elements = Arc::new(Mutex::new(vec));
    self
  }

  pub fn iter(&mut self) -> QueueIter<E, QueueLinkedList<E>> {
    QueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

impl<E: Element + 'static> HasPeekBehavior<E> for QueueLinkedList<E> {
  async fn peek(&self) -> anyhow::Result<Option<E>> {
    let mg = self.elements.lock().await;
    Ok(mg.front().map(|e| e.clone()))
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for QueueLinkedList<E> {
  async fn contains(&self, element: &E) -> bool {
    let mg = self.elements.lock().await;
    mg.contains(element)
  }
}

impl<E: Element + 'static> IntoIterator for QueueLinkedList<E> {
  type IntoIter = QueueIntoIter<E, QueueLinkedList<E>>;
  type Item = E;

  fn into_iter(self) -> Self::IntoIter {
    QueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

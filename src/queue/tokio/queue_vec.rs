use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::queue::tokio::{HasContainsBehavior, HasPeekBehavior, QueueBehavior};
use crate::queue::{Element, QueueError, QueueIter, QueueSize};

#[derive(Debug, Clone)]
pub struct QueueVec<E> {
  elements: Arc<Mutex<VecDeque<E>>>,
  capacity: QueueSize,
}

impl<E: Element> QueueVec<E> {
  /// Create a new `QueueVec`.<br/>
  /// 新しい `QueueVec` を作成します。
  pub fn new() -> Self {
    Self {
      elements: Arc::new(Mutex::new(VecDeque::new())),
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
  /// - `elements` - The elements to be updated. / 更新する要素。
  pub fn with_elements(mut self, elements: impl IntoIterator<Item = E>) -> Self {
    let vec = elements.into_iter().collect::<VecDeque<E>>();
    self.elements = Arc::new(Mutex::new(vec));
    self
  }

  pub fn iter(&mut self) -> QueueIter<E, QueueVec<E>> {
    QueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static> QueueBehavior<E> for QueueVec<E> {
  async fn len(&self) -> QueueSize {
    let mg = self.elements.lock().await;
    let len = mg.len();
    QueueSize::Limited(len)
  }

  async fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  async fn offer(&mut self, element: E) -> anyhow::Result<()> {
    if self.non_full().await {
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

#[async_trait::async_trait]
impl<E: Element + 'static> HasPeekBehavior<E> for QueueVec<E> {
  async fn peek(&self) -> anyhow::Result<Option<E>> {
    let mg = self.elements.lock().await;
    Ok(mg.front().map(|e| e.clone()))
  }
}

#[async_trait::async_trait]
impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for QueueVec<E> {
  async fn contains(&self, element: &E) -> bool {
    let mg = self.elements.lock().await;
    mg.contains(element)
  }
}

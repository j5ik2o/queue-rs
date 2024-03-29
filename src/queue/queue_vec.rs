use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::queue::{
  Element, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueError, QueueIntoIter, QueueIter, QueueSize,
};

/// A queue implementation backed by a `VecDeque`.<br/>
/// `VecDeque` によって実装されたキュー。
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
  pub fn with_elements(mut self, elements: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
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

impl<E: Element + 'static> QueueBehavior<E> for QueueVec<E> {
  fn len(&self) -> QueueSize {
    let mg = self.elements.lock().unwrap();
    let len = mg.len();
    QueueSize::Limited(len)
  }

  fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  fn offer(&mut self, element: E) -> Result<()> {
    if self.non_full() {
      let mut mg = self.elements.lock().unwrap();
      mg.push_back(element);
      Ok(())
    } else {
      Err(QueueError::OfferError(element).into())
    }
  }

  fn poll(&mut self) -> Result<Option<E>> {
    let mut mg = self.elements.lock().unwrap();
    Ok(mg.pop_front())
  }
}

impl<E: Element + 'static> HasPeekBehavior<E> for QueueVec<E> {
  fn peek(&self) -> Result<Option<E>> {
    let mg = self.elements.lock().unwrap();
    Ok(mg.front().map(|e| e.clone()))
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for QueueVec<E> {
  fn contains(&self, element: &E) -> bool {
    let mg = self.elements.lock().unwrap();
    mg.contains(element)
  }
}

impl<E: Element + 'static> IntoIterator for QueueVec<E> {
  type IntoIter = QueueIntoIter<E, QueueVec<E>>;
  type Item = E;

  fn into_iter(self) -> Self::IntoIter {
    QueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

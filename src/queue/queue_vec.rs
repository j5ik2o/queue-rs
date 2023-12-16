use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::queue::{
  Element, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueError, QueueIntoIter, QueueIter, QueueSize,
};

/// A queue implementation backed by a `VecDeque`.<br/>
/// `VecDeque` で実装されたキュー。
#[derive(Debug, Clone)]
pub struct QueueVec<E> {
  values: Arc<Mutex<VecDeque<E>>>,
  pub(crate) capacity: QueueSize,
}

impl<E: Element> QueueVec<E> {
  pub fn new() -> Self {
    Self {
      values: Arc::new(Mutex::new(VecDeque::new())),
      capacity: QueueSize::Limitless,
    }
  }

  pub fn with_capacity(mut self, capacity: QueueSize) -> Self {
    self.capacity = capacity;
    self
  }

  pub fn with_elements(mut self, values: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let vec = values.into_iter().collect::<VecDeque<E>>();
    self.values = Arc::new(Mutex::new(vec));
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
    let mg = self.values.lock().unwrap();
    let len = mg.len();
    QueueSize::Limited(len)
  }

  fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  fn offer(&mut self, element: E) -> Result<()> {
    if self.non_full() {
      let mut mg = self.values.lock().unwrap();
      mg.push_back(element);
      Ok(())
    } else {
      Err(QueueError::OfferError(element).into())
    }
  }

  fn poll(&mut self) -> Result<Option<E>> {
    let mut mg = self.values.lock().unwrap();
    Ok(mg.pop_front())
  }
}

impl<E: Element + 'static> HasPeekBehavior<E> for QueueVec<E> {
  fn peek(&self) -> Result<Option<E>> {
    let mg = self.values.lock().unwrap();
    Ok(mg.front().map(|e| e.clone()))
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for QueueVec<E> {
  fn contains(&self, element: &E) -> bool {
    let mg = self.values.lock().unwrap();
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

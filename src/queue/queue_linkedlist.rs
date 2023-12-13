use crate::queue::{Element, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueError, QueueSize};
use std::collections::LinkedList;
use std::sync::{Arc, Mutex};

/// A queue implementation backed by a `LinkedList`.<br/>
/// `LinkedList` で実装されたキュー。
#[derive(Debug, Clone)]
pub struct QueueLinkedList<E> {
  values: Arc<Mutex<LinkedList<E>>>,
  pub(crate) capacity: QueueSize,
}

impl<E: Element + 'static> QueueBehavior<E> for QueueLinkedList<E> {
  fn len(&self) -> QueueSize {
    let mg = self.values.lock().unwrap();
    let len = mg.len();
    QueueSize::Limited(len)
  }

  fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  fn offer(&mut self, element: E) -> anyhow::Result<()> {
    if self.non_full() {
      let mut mg = self.values.lock().unwrap();
      mg.push_back(element);
      Ok(())
    } else {
      Err(QueueError::OfferError(element).into())
    }
  }

  fn poll(&mut self) -> anyhow::Result<Option<E>> {
    let mut mg = self.values.lock().unwrap();
    Ok(mg.pop_front())
  }
}

impl<E: Element + 'static> QueueLinkedList<E> {
  pub fn new() -> Self {
    Self {
      values: Arc::new(Mutex::new(LinkedList::new())),
      capacity: QueueSize::Limitless,
    }
  }

  pub fn with_num_elements(mut self, num_elements: usize) -> Self {
    self.capacity = QueueSize::Limited(num_elements);
    self
  }

  pub fn with_elements(mut self, values: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let num_elements = values.len();
    let vec = values.into_iter().collect::<LinkedList<E>>();
    self.values = Arc::new(Mutex::new(vec));
    self.capacity = QueueSize::Limited(num_elements);
    self
  }
}

impl<E: Element + 'static> HasPeekBehavior<E> for QueueLinkedList<E> {
  fn peek(&self) -> anyhow::Result<Option<E>> {
    let mg = self.values.lock().unwrap();
    Ok(mg.front().map(|e| e.clone()))
  }
}

impl<E: Element + PartialEq + 'static> HasContainsBehavior<E> for QueueLinkedList<E> {
  fn contains(&self, element: &E) -> bool {
    let mg = self.values.lock().unwrap();
    mg.contains(element)
  }
}

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use thiserror::Error;

use crate::queue::{QueueSize, QueueBehavior, QueueError, HasPeekBehavior};

#[derive(Debug, Clone)]
pub struct QueueVec<E> {
  values: VecDeque<E>,
  pub(crate) capacity: QueueSize,
}

impl<E> QueueVec<E> {
  pub fn new() -> Self {
    Self {
      values: VecDeque::new(),
      capacity: QueueSize::Limitless,
    }
  }

  pub fn with_num_elements(num_elements: usize) -> Self {
    Self {
      values: VecDeque::new(),
      capacity: QueueSize::Limited(num_elements),
    }
  }

  pub fn with_elements(values: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let num_elements = values.len();
    let vec = values.into_iter().collect::<VecDeque<E>>();
    Self {
      values: vec,
      capacity: QueueSize::Limited(num_elements),
    }
  }
}

impl<E: Debug + Clone + Send + Sync + 'static> QueueBehavior<E> for QueueVec<E> {
  fn len(&self) -> QueueSize {
    QueueSize::Limited(self.values.len())
  }

  fn capacity(&self) -> QueueSize {
    self.capacity.clone()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    if self.non_full() {
      self.values.push_back(e);
      Ok(())
    } else {
      Err(anyhow::Error::new(QueueError::OfferError(e)))
    }
  }

  fn poll(&mut self) -> Result<Option<E>> {
    Ok(self.values.pop_front())
  }
}

impl<E: Debug + Clone + Send + Sync + 'static> HasPeekBehavior<E> for QueueVec<E> {
  fn peek(&self) -> Result<Option<E>> {
    Ok(self.values.front().map(|e| e.clone()))
  }
}

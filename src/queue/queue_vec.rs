use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use thiserror::Error;

use crate::queue::{Capacity, QueueBehavior, QueueError};

#[derive(Debug, Clone)]
pub struct QueueVec<E> {
  values: VecDeque<E>,
  pub(crate) capacity: Capacity,
}

impl<E> QueueVec<E> {
  pub fn new() -> Self {
    Self {
      values: VecDeque::new(),
      capacity: Capacity::Limitless,
    }
  }

  pub fn with_num_elements(num_elements: usize) -> Self {
    Self {
      values: VecDeque::new(),
      capacity: Capacity::Limited(num_elements),
    }
  }

  pub fn with_elements(values: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let num_elements = values.len();
    let vec = values.into_iter().collect::<VecDeque<E>>();
    Self {
      values: vec,
      capacity: Capacity::Limited(num_elements),
    }
  }
}

impl<E: Debug + Clone + Send + Sync + 'static> QueueBehavior<E> for QueueVec<E> {
  fn len(&self) -> usize {
    self.values.len()
  }

  fn capacity(&self) -> Capacity {
    self.capacity.clone()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    if self.capacity >= self.len() + 1 {
      self.values.push_back(e);
      Ok(())
    } else {
      Err(anyhow::Error::new(QueueError::OfferError(e)))
    }
  }

  fn poll(&mut self) -> Option<E> {
    self.values.pop_front()
  }

  fn peek(&self) -> Option<E> {
    self.values.front().map(|e| e.clone())
  }
}

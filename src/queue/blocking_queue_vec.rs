use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::Future;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use thiserror::Error;
use crate::queue::{BlockingQueueBehavior, QueueBehavior, QueueVec};

#[derive(Debug, Clone)]
pub struct BlockingQueueVec<E> {
  underlying: Arc<(Mutex<QueueVec<E>>, Condvar, Condvar)>,
}

impl<E: Debug + Clone + Sync + Send + 'static> QueueBehavior<E> for BlockingQueueVec<E> {
  fn len(&self) -> usize {
    let (qg, _, _) = &*self.underlying;
    let lq = qg.lock().unwrap();
    lq.len()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    let (qg, _, not_empty) = &*self.underlying;
    let mut lq = qg.lock().unwrap();
    let result = lq.offer(e);
    not_empty.notify_one();
    result
  }

  fn poll(&mut self) -> Option<E> {
    let (qg, not_full, _) = &*self.underlying;
    let mut lq = qg.lock().unwrap();
    let result = lq.poll();
    not_full.notify_one();
    result
  }

  fn peek(&self) -> Option<E> {
    let (qg, not_full, _) = &*self.underlying;
    let lq = qg.lock().unwrap();
    let result = lq.peek();
    not_full.notify_one();
    result
  }
}

impl<E: Debug + Clone + Sync + Send + 'static> BlockingQueueBehavior<E> for BlockingQueueVec<E> {
  fn put(&mut self, e: E) -> Result<()> {
    let (qg, not_full, not_empty) = &*self.underlying;
    let mut lq = qg.lock().unwrap();
    while lq.len() == lq.num_elements {
      lq = not_full.wait(lq).unwrap();
    }
    let result = lq.offer(e);
    not_empty.notify_one();
    result
  }

  fn take(&mut self) -> Option<E> {
    let (qg, not_full, not_empty) = &*self.underlying;
    let mut lq = qg.lock().unwrap();
    while lq.len() == 0 {
      lq = not_empty.wait(lq).unwrap();
    }
    let result = lq.poll();
    not_full.notify_one();
    result
  }
}

impl<E: Debug + Send + Sync + 'static> BlockingQueueVec<E> {
  pub fn new() -> Self {
    Self {
      underlying: Arc::new((
        Mutex::new(QueueVec::with_num_elements(32)),
        Condvar::new(),
        Condvar::new(),
      )),
    }
  }

  pub fn with_num_elements(num_elements: usize) -> Self {
    Self {
      underlying: Arc::new((
        Mutex::new(QueueVec::with_num_elements(num_elements)),
        Condvar::new(),
        Condvar::new(),
      )),
    }
  }
}

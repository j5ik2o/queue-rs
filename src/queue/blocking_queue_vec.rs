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
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    queue_vec_mutex_guard.len()
  }

  fn num_elements(&self) -> usize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    queue_vec_mutex_guard.num_elements()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.offer(e);
    not_empty.notify_one();
    result
  }

  fn poll(&mut self) -> Option<E> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.poll();
    not_full.notify_one();
    result
  }

  fn peek(&self) -> Option<E> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.peek();
    not_full.notify_one();
    result
  }
}

impl<E: Debug + Clone + Sync + Send + 'static> BlockingQueueBehavior<E> for BlockingQueueVec<E> {
  fn put(&mut self, e: E) -> Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_full() {
      queue_vec_mutex_guard = not_full.wait(queue_vec_mutex_guard).unwrap();
    }
    let result = queue_vec_mutex_guard.offer(e);
    not_empty.notify_one();
    result
  }

  fn take(&mut self) -> Option<E> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_empty() {
      queue_vec_mutex_guard = not_empty.wait(queue_vec_mutex_guard).unwrap();
    }
    let result = queue_vec_mutex_guard.poll();
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

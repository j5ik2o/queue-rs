use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::Result;

use crate::queue::{BlockingQueueBehavior, Element, QueueBehavior, QueueSize};

#[derive(Debug, Clone)]
pub struct BlockingQueue<E, Q: QueueBehavior<E>> {
  underlying: Arc<(Mutex<Q>, Condvar, Condvar)>,
  p: PhantomData<E>,
  is_interrupted: Arc<AtomicBool>,
}

impl<E: Element + 'static, Q: QueueBehavior<E>> QueueBehavior<E> for BlockingQueue<E, Q> {
  fn len(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    queue_vec_mutex_guard.len()
  }

  fn capacity(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    queue_vec_mutex_guard.capacity()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.offer(e);
    not_empty.notify_one();
    result
  }

  fn poll(&mut self) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.poll();
    not_full.notify_one();
    result
  }

  // fn peek(&self) -> Result<Option<E>> {
  //   let (queue_vec_mutex, not_full, _) = &*self.underlying;
  //   let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
  //   let result = queue_vec_mutex_guard.peek();
  //   not_full.notify_one();
  //   result
  // }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> BlockingQueueBehavior<E> for BlockingQueue<E, Q> {
  fn put(&mut self, e: E) -> Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_full() {
      match self
        .is_interrupted
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
      {
        Ok(_) => {
          // 中断されたときの処理
          return Err(anyhow::anyhow!("interrupted"));
        }
        Err(_) => {
          // 中断されていないときの処理
        }
      }
      log::debug!("put: blocking start...");
      queue_vec_mutex_guard = not_full.wait(queue_vec_mutex_guard).unwrap();
      log::debug!("put: blocking end...");
    }
    let result = queue_vec_mutex_guard.offer(e);
    not_empty.notify_one();
    result
  }

  fn take(&mut self) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_empty() {
      match self
        .is_interrupted
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
      {
        Ok(_) => {
          // 中断されたときの処理
          return Err(anyhow::anyhow!("interrupted"));
        }
        Err(_) => {
          // 中断されていないときの処理
        }
      }
      log::debug!("take: blocking start...");
      queue_vec_mutex_guard = not_empty.wait(queue_vec_mutex_guard).unwrap();
      log::debug!("take: blocking end...");
    }
    let result = queue_vec_mutex_guard.poll();
    not_full.notify_one();
    result
  }

  fn interrupt(&mut self) {
    self.is_interrupted.store(true, Ordering::Relaxed);
    let (_, not_full, not_empty) = &*self.underlying;
    not_empty.notify_all();
    not_full.notify_all();
  }

  fn is_interrupted(&self) -> bool {
    self.is_interrupted.load(Ordering::Relaxed)
  }
}

impl<E, Q: QueueBehavior<E>> BlockingQueue<E, Q> {
  pub fn new(queue: Q) -> Self {
    Self {
      underlying: Arc::new((Mutex::new(queue), Condvar::new(), Condvar::new())),
      p: PhantomData::default(),
      is_interrupted: Arc::new(AtomicBool::new(false)),
    }
  }
}

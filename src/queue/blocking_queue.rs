use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::Result;

use crate::queue::{
  BlockingQueueBehavior, Element, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueError, QueueIntoIter,
  QueueIter, QueueSize,
};

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

  fn offer(&mut self, elements: E) -> Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.offer(elements);
    not_empty.notify_one();
    result
  }

  fn offer_all(&mut self, elements: impl IntoIterator<Item = E>) -> Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.offer_all(elements);
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
}

impl<E: Element + 'static, Q: QueueBehavior<E> + HasPeekBehavior<E>> HasPeekBehavior<E> for BlockingQueue<E, Q> {
  fn peek(&self) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.peek();
    not_full.notify_one();
    result
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E> + HasContainsBehavior<E>> HasContainsBehavior<E>
  for BlockingQueue<E, Q>
{
  fn contains(&self, element: &E) -> bool {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.contains(element);
    result
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> BlockingQueueBehavior<E> for BlockingQueue<E, Q> {
  fn put(&mut self, element: E) -> Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_full() {
      if self.check_and_update_interrupted() {
        log::debug!("put: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("put: blocking start...");
      queue_vec_mutex_guard = not_full.wait(queue_vec_mutex_guard).unwrap();
      log::debug!("put: blocking end...");
    }
    let result = queue_vec_mutex_guard.offer(element);
    not_empty.notify_one();
    result
  }

  fn put_timeout(&mut self, element: E, timeout: Duration) -> Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_full() {
      if self.check_and_update_interrupted() {
        log::debug!("put: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("put: blocking start...");
      let (mg, wtr) = not_full.wait_timeout(queue_vec_mutex_guard, timeout).unwrap();
      if wtr.timed_out() {
        log::debug!("put: blocking timeout...");
        return Err(QueueError::<E>::TimeoutError.into());
      }
      queue_vec_mutex_guard = mg;
      log::debug!("put: blocking end...");
    }
    let result = queue_vec_mutex_guard.offer(element);
    not_empty.notify_one();
    result
  }

  fn take(&mut self) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_empty() {
      if self.check_and_update_interrupted() {
        log::debug!("take: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("take: blocking start...");
      queue_vec_mutex_guard = not_empty.wait(queue_vec_mutex_guard).unwrap();
      log::debug!("take: blocking end...");
    }
    let result = queue_vec_mutex_guard.poll();
    not_full.notify_one();
    result
  }

  fn take_timeout(&mut self, timeout: Duration) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_empty() {
      if self.check_and_update_interrupted() {
        log::debug!("take: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("take: blocking start...");
      let (mg, wtr) = not_empty.wait_timeout(queue_vec_mutex_guard, timeout).unwrap();
      if wtr.timed_out() {
        log::debug!("take: blocking timeout...");
        return Err(QueueError::<E>::TimeoutError.into());
      }
      queue_vec_mutex_guard = mg;
      log::debug!("take: blocking end...");
    }
    let result = queue_vec_mutex_guard.poll();
    not_full.notify_one();
    result
  }

  fn remaining_capacity(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let capacity = queue_vec_mutex_guard.capacity();
    let len = queue_vec_mutex_guard.len();
    match (capacity.clone(), len.clone()) {
      (QueueSize::Limited(capacity), QueueSize::Limited(len)) => QueueSize::Limited(capacity - len),
      (QueueSize::Limitless, _) => QueueSize::Limitless,
      (_, _) => QueueSize::Limited(0),
    }
  }

  fn interrupt(&mut self) {
    log::debug!("interrupt: start...");
    self.is_interrupted.store(true, Ordering::Relaxed);
    let (_, not_full, not_empty) = &*self.underlying;
    not_empty.notify_all();
    not_full.notify_all();
    log::debug!("interrupt: end...");
  }

  fn is_interrupted(&self) -> bool {
    self.is_interrupted.load(Ordering::Relaxed)
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> BlockingQueue<E, Q> {
  pub fn new(queue: Q) -> Self {
    Self {
      underlying: Arc::new((Mutex::new(queue), Condvar::new(), Condvar::new())),
      p: PhantomData::default(),
      is_interrupted: Arc::new(AtomicBool::new(false)),
    }
  }

  fn check_and_update_interrupted(&self) -> bool {
    match self
      .is_interrupted
      .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
    {
      Ok(_) => true,
      Err(_) => false,
    }
  }

  pub fn iter(&mut self) -> QueueIter<E, BlockingQueue<E, Q>> {
    QueueIter {
      q: self,
      p: PhantomData,
    }
  }

  pub fn blocking_iter(&mut self) -> BlockingQueueIter<E, Q> {
    BlockingQueueIter {
      q: self,
      p: PhantomData,
    }
  }

  pub fn into_blocking_iter(self) -> BlockingQueueIntoIter<E, Q> {
    BlockingQueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> IntoIterator for BlockingQueue<E, Q> {
  type IntoIter = QueueIntoIter<E, BlockingQueue<E, Q>>;
  type Item = E;

  fn into_iter(self) -> Self::IntoIter {
    QueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

pub struct BlockingQueueIntoIter<E: Element + 'static, Q: QueueBehavior<E>> {
  q: BlockingQueue<E, Q>,
  p: PhantomData<E>,
}

impl<E: Element + 'static, Q: QueueBehavior<E>> Iterator for BlockingQueueIntoIter<E, Q> {
  type Item = E;

  fn next(&mut self) -> Option<Self::Item> {
    self.q.take().ok().flatten()
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> ExactSizeIterator for BlockingQueueIntoIter<E, Q> {
  fn len(&self) -> usize {
    self.q.len().to_usize()
  }
}

pub struct BlockingQueueIter<'a, E: Element + 'static, Q: QueueBehavior<E>> {
  q: &'a mut BlockingQueue<E, Q>,
  p: PhantomData<E>,
}

impl<'a, E: Element + 'static, Q: QueueBehavior<E>> Iterator for BlockingQueueIter<'a, E, Q> {
  type Item = E;

  fn next(&mut self) -> Option<Self::Item> {
    self.q.take().ok().flatten()
  }
}

impl<'a, E: Element + 'static, Q: QueueBehavior<E>> ExactSizeIterator for BlockingQueueIter<'a, E, Q> {
  fn len(&self) -> usize {
    self.q.len().to_usize()
  }
}

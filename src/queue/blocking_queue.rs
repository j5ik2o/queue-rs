use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::Result;

use crate::queue::{
  BlockingQueueBehavior, Element, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueError, QueueSize,
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
  fn contains(&self, element: &E) -> Result<bool> {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    let result = queue_vec_mutex_guard.contains(element);
    result
  }
}

impl<E: Element + 'static, Q: QueueBehavior<E>> BlockingQueueBehavior<E> for BlockingQueue<E, Q> {
  fn put(&mut self, e: E) -> Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_full() {
      if self.check_and_update_interrupted() {
        // log::debug!("put: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      // log::debug!("put: blocking start...");
      queue_vec_mutex_guard = not_full.wait(queue_vec_mutex_guard).unwrap();
      // log::debug!("put: blocking end...");
    }
    let result = queue_vec_mutex_guard.offer(e);
    not_empty.notify_one();
    result
  }

  fn take(&mut self) -> Result<Option<E>> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().unwrap();
    while queue_vec_mutex_guard.is_empty() {
      if self.check_and_update_interrupted() {
        // log::debug!("take: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      // log::debug!("take: blocking start...");
      queue_vec_mutex_guard = not_empty.wait(queue_vec_mutex_guard).unwrap();
      // log::debug!("take: blocking end...");
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
    // log::debug!("interrupting...");
    self.is_interrupted.store(true, Ordering::Relaxed);
    let (_, not_full, not_empty) = &*self.underlying;
    not_empty.notify_one();
    not_full.notify_one();
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

  fn check_and_update_interrupted(&self) -> bool {
    match self
      .is_interrupted
      .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
    {
      Ok(_) => true,
      Err(_) => false,
    }
  }
}

#[cfg(test)]
mod tests {
  use serial_test::serial;
  use std::sync::{Arc, Condvar, Mutex};
  use std::{env, thread};

  use crate::queue::{create_queue, BlockingQueue, HasContainsBehavior, Queue, QueueBehavior, QueueType};
  use crate::queue::{BlockingQueueBehavior, QueueSize};
  extern crate env_logger;

  fn init_logger() {
    env::set_var("RUST_LOG", "debug");
    // env::set_var("RUST_LOG", "trace");
    let _ = env_logger::try_init();
  }

  struct CountDownLatch {
    count: Mutex<usize>,
    condvar: Condvar,
  }

  impl CountDownLatch {
    fn new(count: usize) -> Self {
      CountDownLatch {
        count: Mutex::new(count),
        condvar: Condvar::new(),
      }
    }

    fn count_down(&self) {
      let mut count = self.count.lock().unwrap();
      *count -= 1;
      if *count == 0 {
        self.condvar.notify_all();
      }
    }

    fn wait(&self) {
      let mut count = self.count.lock().unwrap();
      while *count > 0 {
        count = self.condvar.wait(count).unwrap();
      }
    }
  }

  const QUEUE_SIZE: usize = 10;

  #[test]
  #[serial]
  fn test_empty_full() {
    init_logger();
    let size: usize = QUEUE_SIZE;
    let mut q = create_queue::<i32>(QueueType::Vec, Some(2)).with_blocking();

    assert!(q.is_empty());
    assert_eq!(2, q.remaining_capacity().unwrap());
    assert!(q.offer(1).is_ok());
    assert!(!q.is_empty());
    assert!(q.offer(2).is_ok());
    assert!(!q.is_empty());
    assert_eq!(0, q.remaining_capacity().unwrap());
    assert!(q.offer(3).is_err());
  }

  #[test]
  #[serial]
  fn test_remaining_capacity() {
    init_logger();
    let size: usize = QUEUE_SIZE;
    let mut q = populated_queue(QueueType::Vec, size);
    for i in 0..size {
      let remaining_capacity = q.remaining_capacity().unwrap();
      let len = q.len().unwrap();
      assert_eq!(remaining_capacity, i);
      assert_eq!(size, len + remaining_capacity);
      assert!(q.take().is_ok());
    }
    for i in 0..size {
      let remaining_capacity = q.remaining_capacity().unwrap();
      let len = q.len().unwrap();
      assert_eq!(remaining_capacity, size - i);
      assert_eq!(size, len + remaining_capacity);
      assert!(q.offer(i as i32).is_ok());
    }
  }

  #[test]
  #[serial]
  fn test_offer() {
    init_logger();
    let mut q = create_queue(QueueType::Vec, Some(1)).with_blocking();
    assert!(q.offer(0).is_ok());
    assert!(q.offer(1).is_err());
  }

  #[test]
  #[serial]
  fn test_put() {
    init_logger();
    let size: usize = QUEUE_SIZE;

    let mut q = create_queue(QueueType::Vec, Some(size)).with_blocking();
    for i in 0..size {
      q.put(i as i32).unwrap();
      assert!(q.contains(&(i as i32)).unwrap());
    }
  }

  #[test]
  #[serial]
  fn test_blocking_put() {
    init_logger();
    let size: usize = QUEUE_SIZE;
    let mut bqv1 = create_queue(QueueType::Vec, Some(size)).with_blocking();
    let mut bqv2 = bqv1.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..size {
        // log::debug!("take: start: {}", i);
        let _ = bqv2.put(i as i32).unwrap();
        // log::debug!("take: finish: {},{:?}", i, n);
      }
      assert_eq!(bqv2.len(), QueueSize::Limited(size));

      bqv2.interrupt();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(_) => {
          // log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());

      please_interrupt_cloned.count_down();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(_) => {
          // log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());
    });

    please_interrupt.wait();
    bqv1.interrupt();
    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  fn test_put_with_take() {
    init_logger();
    let capacity = 2;

    let mut bqv1 = create_queue(QueueType::Vec, Some(capacity)).with_blocking();
    let mut bqv2 = bqv1.clone();

    let please_take = Arc::new(CountDownLatch::new(1));
    let please_take_cloned = please_take.clone();
    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..capacity {
        // log::debug!("take: start: {}", i);
        let _ = bqv2.put(i);
        // log::debug!("take: finish: {},{:?}", i, n);
      }
      please_take_cloned.count_down();
      bqv2.put(86).unwrap();

      please_interrupt_cloned.count_down();
      match bqv2.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(_) => {
          // log::debug!("put: finish: 99, error = {:?}", e);
        }
      }
    });

    please_take.wait();

    let r = bqv1.take().unwrap();
    assert_eq!(r, Some(0));

    please_interrupt.wait();
    bqv1.interrupt();

    handler1.join().unwrap();
  }

  fn populated_queue(queue_type: QueueType, size: usize) -> BlockingQueue<i32, Queue<i32>> {
    let mut q = create_queue::<i32>(queue_type, Some(size)).with_blocking();
    for i in 0..size {
      q.offer(i as i32).unwrap();
    }
    q
  }

  #[test]
  #[serial]
  pub fn test_blocking_take() {
    init_logger();
    let size: usize = QUEUE_SIZE;
    let mut bqv1 = populated_queue(QueueType::Vec, size);
    let mut bqv2 = bqv1.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for _ in 0..size {
        // log::debug!("take: start: {}", i);
        let _ = bqv2.take().unwrap();
        // log::debug!("take: finish: {},{:?}", i, n);
      }

      bqv2.interrupt();
      match bqv2.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(_) => {
          // log::debug!("take: finish: error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());

      please_interrupt_cloned.count_down();
      match bqv2.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(_) => {
          // log::debug!("take: finish: error = {:?}", e);
        }
      }
      assert!(!bqv2.is_interrupted());
    });

    please_interrupt.wait();
    bqv1.interrupt();
    handler1.join().unwrap();
  }
}

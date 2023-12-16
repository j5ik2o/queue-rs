use crate::queue::{BlockingQueueBehavior, QueueError, QueueSize};
use std::time::Duration;

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Condvar, Mutex};
  use std::{env, thread};

  use serial_test::serial;

  use crate::queue::{
    create_queue, create_queue_with_elements, BlockingQueue, HasContainsBehavior, Queue, QueueBehavior, QueueType,
  };

  use super::*;

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
      Self {
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

  const QUEUE_SIZE: QueueSize = QueueSize::Limited(10);
  const TIME_OUT: Duration = Duration::from_millis(100);

  #[test]
  #[serial]
  fn test_constructor1() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    assert_eq!(q.remaining_capacity(), QUEUE_SIZE);
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_constructor2() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limitless);
    assert_eq!(QueueSize::Limitless, q.remaining_capacity());
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_constructor3() {
    init_logger();

    let elements: Vec<i32> = Vec::with_capacity(QUEUE_SIZE.to_usize());
    let q = create_blocking_queue_with_elements(
      QueueType::VecDequeue,
      QueueSize::Limited(elements.len()),
      elements.clone().into_iter(),
    );
    assert_eq!(q.remaining_capacity(), QueueSize::Limited(elements.len()));
  }

  #[test]
  #[serial]
  fn test_constructor4() {
    init_logger();

    let elements = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    let q = create_blocking_queue_with_elements(QueueType::VecDequeue, elements.len(), elements.clone().into_iter());
    assert_eq!(q.remaining_capacity(), QueueSize::Limited(elements.len().to_usize()));
  }

  #[test]
  #[serial]
  fn test_iter() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for e in q.iter() {
      log::debug!("e = {:?}", e);
    }
    assert_eq!(q.poll().unwrap(), None);
    assert_eq!(q.len(), QueueSize::Limited(0));
  }

  #[test]
  #[serial]
  fn test_into_iter() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for e in q.clone().into_iter() {
      log::debug!("e = {:?}", e);
    }
    assert_eq!(q.poll().unwrap(), None);
    assert_eq!(q.len(), QueueSize::Limited(0));
  }

  #[test]
  #[serial]
  fn test_blocking_iter() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    let mut q_cloned = q.clone();
    let mut counter = q.len().to_usize();
    for e in q.blocking_iter() {
      log::debug!("{}: e = {:?}", counter, e);
      if counter == 1 {
        q_cloned.interrupt();
      }
      counter -= 1;
    }
    assert_eq!(q_cloned.len(), QueueSize::Limited(0));
  }

  #[test]
  #[serial]
  fn test_into_blocking_iter() {
    init_logger();

    let q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    let mut q_cloned = q.clone();
    let mut counter = q.len().to_usize();
    for e in q.into_blocking_iter() {
      log::debug!("{}: e = {:?}", counter, e);
      if counter == 1 {
        q_cloned.interrupt();
      }
      counter -= 1;
    }
    assert_eq!(q_cloned.len(), QueueSize::Limited(0));
  }

  #[test]
  #[serial]
  fn test_empty_full() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limited(2));
    assert!(q.is_empty());
    assert_eq!(q.remaining_capacity().to_usize(), 2);
    assert!(q.offer(1).is_ok());
    assert!(!q.is_empty());
    assert!(q.offer(2).is_ok());
    assert!(!q.is_empty());
    assert_eq!(q.remaining_capacity().to_usize(), 0);
    assert!(q.offer(3).is_err());

    assert_eq!(q.poll().unwrap(), Some(1));
    assert_eq!(q.poll().unwrap(), Some(2));
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_remaining_capacity() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for i in 0..QUEUE_SIZE.to_usize() {
      let remaining_capacity = q.remaining_capacity().to_usize();
      let len = q.len().to_usize();
      assert_eq!(remaining_capacity, i);
      assert_eq!(len + remaining_capacity, QUEUE_SIZE.to_usize());
      assert!(q.take().is_ok());
    }
    for i in 0..QUEUE_SIZE.to_usize() {
      let remaining_capacity = q.remaining_capacity().to_usize();
      let len = q.len().to_usize();
      assert_eq!(remaining_capacity, QUEUE_SIZE.to_usize() - i);
      assert_eq!(len + remaining_capacity, QUEUE_SIZE.to_usize());
      assert!(q.offer(i as i32).is_ok());
    }
    for i in 0..QUEUE_SIZE.to_usize() {
      assert_eq!(q.poll().unwrap(), Some(i as i32));
    }
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_offer() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limited(1));
    assert!(q.offer(0).is_ok());
    assert!(q.offer(1).is_err());

    assert_eq!(q.poll().unwrap(), Some(0));
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_offer_all() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limited(10));
    let v = [1, 2, 3, 4, 5];

    assert!(q.offer_all(v).is_ok());

    assert_eq!(q.poll().unwrap(), Some(1));
    assert_eq!(q.poll().unwrap(), Some(2));
    assert_eq!(q.poll().unwrap(), Some(3));
    assert_eq!(q.poll().unwrap(), Some(4));
    assert_eq!(q.poll().unwrap(), Some(5));
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_put() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for i in 0..QUEUE_SIZE.to_usize() {
      q.put(i as i32).unwrap();
      assert!(q.contains(&(i as i32)));
    }
  }

  #[test]
  #[serial]
  fn test_blocking_put() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    let mut q_cloned = q.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..QUEUE_SIZE.to_usize() {
        let _ = q_cloned.put(i as i32).unwrap();
      }
      assert_eq!(q_cloned.len(), QUEUE_SIZE);

      q_cloned.interrupt();
      match q_cloned.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(err) => {
          log::debug!("put: finish: 99, error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
      assert!(!q_cloned.is_interrupted());

      please_interrupt_cloned.count_down();
      match q_cloned.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(err) => {
          log::debug!("put: finish: 99, error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
      assert!(!q_cloned.is_interrupted());
    });

    please_interrupt.wait();
    assert!(!handler1.is_finished());
    q.interrupt();
    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  fn test_put_with_take() {
    init_logger();

    let capacity = 2;

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limited(capacity));
    let mut q_cloned = q.clone();

    let please_take = Arc::new(CountDownLatch::new(1));
    let please_take_cloned = please_take.clone();
    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for i in 0..capacity {
        let _ = q_cloned.put(i as i32);
      }
      please_take_cloned.count_down();
      q_cloned.put(86).unwrap();

      please_interrupt_cloned.count_down();
      match q_cloned.put(99) {
        Ok(_) => {
          panic!("put: finish: 99, should not be here");
        }
        Err(err) => {
          log::debug!("put: finish: 99, error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
    });

    please_take.wait();
    assert_eq!(q.take().unwrap(), Some(0));

    please_interrupt.wait();
    q.interrupt();

    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  fn test_timed_put() {
    init_logger();

    let mut q = create_blocking_queue(QueueType::VecDequeue, QueueSize::Limited(2));
    let mut q_cloned = q.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      q_cloned.put(1).unwrap();
      q_cloned.put(2).unwrap();
      let start_time = std::time::Instant::now();
      assert!(q_cloned.put_timeout(3, TIME_OUT).is_err());
      assert!(start_time.elapsed() >= TIME_OUT);

      please_interrupt_cloned.count_down();
      match q_cloned.put_timeout(4, Duration::from_secs(3)) {
        Ok(_) => {
          panic!("put: finish: 4, should not be here");
        }
        Err(err) => {
          log::debug!("put: finish: 4, error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
    });

    please_interrupt.wait();
    assert!(!handler1.is_finished());
    q.interrupt();
    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  pub fn test_blocking_take() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    let mut q_cloned = q.clone();

    let please_interrupt = Arc::new(CountDownLatch::new(1));
    let please_interrupt_cloned = please_interrupt.clone();

    let handler1 = thread::spawn(move || {
      for _ in 0..QUEUE_SIZE.to_usize() {
        let _ = q_cloned.take().unwrap();
      }

      q_cloned.interrupt();
      match q_cloned.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(err) => {
          log::debug!("take: finish: error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
      assert!(!q_cloned.is_interrupted());

      please_interrupt_cloned.count_down();
      match q_cloned.take() {
        Ok(_) => {
          panic!("take: finish: should not be here");
        }
        Err(err) => {
          log::debug!("take: finish: error = {:?}", err);
          let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
          assert_eq!(queue_error, &QueueError::InterruptedError);
        }
      }
      assert!(!q_cloned.is_interrupted());
    });

    please_interrupt.wait();
    assert!(!handler1.is_finished());
    q.interrupt();
    handler1.join().unwrap();
  }

  #[test]
  #[serial]
  fn test_poll() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for i in 0..QUEUE_SIZE.to_usize() {
      assert_eq!(q.poll().unwrap(), Some(i as i32));
    }
    assert_eq!(q.poll().unwrap(), None);
  }

  #[test]
  #[serial]
  fn test_timed_take0() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for i in 0..QUEUE_SIZE.to_usize() {
      assert_eq!(q.take_timeout(Duration::ZERO).unwrap(), Some(i as i32));
    }
    match q.take_timeout(Duration::ZERO) {
      Ok(_) => {
        panic!("take: finish: should not be here");
      }
      Err(err) => {
        log::debug!("take: finish: error = {:?}", err);
        let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
        assert_eq!(queue_error, &QueueError::TimeoutError);
      }
    }
  }

  #[test]
  #[serial]
  fn test_timed_take() {
    init_logger();

    let mut q = populated_blocking_queue(QueueType::VecDequeue, QUEUE_SIZE);
    for i in 0..QUEUE_SIZE.to_usize() {
      let start_time = std::time::Instant::now();
      assert_eq!(q.take_timeout(TIME_OUT).unwrap(), Some(i as i32));
      assert!(start_time.elapsed() <= TIME_OUT);
    }
    let start_time = std::time::Instant::now();
    match q.take_timeout(TIME_OUT) {
      Ok(_) => {
        panic!("take: finish: should not be here");
      }
      Err(err) => {
        log::debug!("take: finish: error = {:?}", err);
        let queue_error = err.downcast_ref::<QueueError<i32>>().unwrap();
        assert_eq!(queue_error, &QueueError::TimeoutError);
      }
    }
    assert!(start_time.elapsed() >= TIME_OUT);
  }

  fn populated_blocking_queue(queue_type: QueueType, size: QueueSize) -> BlockingQueue<i32, Queue<i32>> {
    let mut q = create_blocking_queue(queue_type, size.clone());
    for i in 0..size.to_usize() {
      q.offer(i as i32).unwrap();
    }
    q
  }

  fn create_blocking_queue(queue_type: QueueType, size: QueueSize) -> BlockingQueue<i32, Queue<i32>> {
    let q = create_queue::<i32>(queue_type, size).with_blocking();
    q
  }

  fn create_blocking_queue_with_elements(
    queue_type: QueueType,
    size: QueueSize,
    elements: impl IntoIterator<Item = i32> + ExactSizeIterator,
  ) -> BlockingQueue<i32, Queue<i32>> {
    let q = create_queue_with_elements::<i32>(queue_type, size, elements).with_blocking();
    q
  }
}

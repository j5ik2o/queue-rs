#![feature(generic_associated_types)]
#![feature(associated_type_defaults)]
#[cfg(test)]
extern crate env_logger as logger;

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError<E: Debug + Send + Sync> {
  #[error("Failed to offer an element: {0:?}")]
  OfferError(E),
}

pub trait QueueBehavior<E: Debug + Send + Sync> {
  fn len(&self) -> usize;
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された要素をこのキューに挿入します。
  fn offer(&mut self, e: E) -> Result<()>;
  /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
  fn poll(&mut self) -> Option<E>;
  /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
  fn peek(&self) -> Option<E>;
}

#[derive(Debug, Clone)]
pub struct QueueVec<E> {
  values: VecDeque<E>,
  num_elements: usize,
}

impl<E> QueueVec<E> {
  pub fn new() -> Self {
    Self {
      values: VecDeque::new(),
      num_elements: usize::MAX,
    }
  }

  pub fn with_num_elements(num_elements: usize) -> Self {
    Self {
      values: VecDeque::new(),
      num_elements,
    }
  }

  pub fn with_elements(values: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    let num_elements = values.len();
    let vec = values.into_iter().collect::<VecDeque<E>>();
    Self {
      values: vec,
      num_elements,
    }
  }

  pub fn num_elements(&self) -> usize {
    self.num_elements
  }
}

impl<E: Debug + Clone + Send + Sync + 'static> QueueBehavior<E> for QueueVec<E> {
  fn len(&self) -> usize {
    self.values.len()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    if self.num_elements >= self.values.len() + 1 {
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

pub trait BlockingQueueBehavior<E: Debug + Send + Sync>: QueueBehavior<E> {
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
  fn put(&mut self, e: E) -> Result<()>;
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  fn take(&mut self) -> Option<E>;
}

#[derive(Debug, Clone)]
pub struct BlockingQueueVec<E> {
  underlying: Arc<Mutex<QueueVec<E>>>,
  take: Arc<Mutex<MutexCvar>>,
  put: Arc<Mutex<MutexCvar>>,
}

struct MutexCvar {
  lock: Mutex<bool>,
  cvar: Condvar,
}

impl MutexCvar {
  pub fn new(lock: Mutex<bool>, cvar: Condvar) -> Self {
    Self { lock, cvar }
  }
}

impl<E: Debug + Clone + Sync + Send + 'static> QueueBehavior<E> for BlockingQueueVec<E> {
  fn len(&self) -> usize {
    let lq = self.underlying.lock().unwrap();
    lq.len()
  }

  fn offer(&mut self, e: E) -> Result<()> {
    let mut lq = self.underlying.lock().unwrap();
    lq.offer(e)
  }

  fn poll(&mut self) -> Option<E> {
    let mut lq = self.underlying.lock().unwrap();
    let result = lq.poll();
    result
  }

  fn peek(&self) -> Option<E> {
    let lq = self.underlying.lock().unwrap();
    lq.peek()
  }
}

impl<E: Debug + Clone + Sync + Send + 'static> BlockingQueueBehavior<E> for BlockingQueueVec<E> {
  fn put(&mut self, e: E) -> Result<()> {
    loop {
      let underlying = self.underlying.lock().unwrap();
      log::debug!(
        "len = {}, num_elements = {}",
        underlying.len(),
        underlying.num_elements
      );
      if underlying.len() < underlying.num_elements {
        break;
      }
      drop(underlying);
      {
        let mut p = self.put.lock().unwrap();
        log::debug!("put_cvar#wait..");
        let mut pl = p.lock.lock().unwrap();
        let _ = p.cvar.wait_while(pl, |pl| !**pl).unwrap();
      }
    }
    let mut underlying = self.underlying.lock().unwrap();
    underlying.offer(e);
    log::debug!("start: take_cvar#notify_one");
    let mut pl = self.put_lock.lock().unwrap();
    *pl = true;
    self.take_cvar.notify_one();
    log::debug!("finish: take_cvar#notify_one");
    Ok(())
  }

  fn take(&mut self) -> Option<E> {
    loop {
      let underlying = self.underlying.lock().unwrap();
      log::debug!(
        "len = {}, num_elements = {}",
        underlying.len(),
        underlying.num_elements
      );
      if underlying.len() > 0 {
        break;
      }
      drop(underlying);
      {
        let mut tl = self.take_lock.lock().unwrap();
        log::debug!("take_cvar#wait..");
        let _ = self.take_cvar.wait_while(tl, |tl| !*tl).unwrap();
      }
    }
    let mut underlying = self.underlying.lock().unwrap();
    let result = underlying.poll();
    log::debug!("start: put_cvar#notify_one");
    let mut l = self.put_lock.lock().unwrap();
    *l = true;
    self.put_cvar.notify_one();
    log::debug!("finish: put_cvar#notify_one");
    result
  }
}

impl<E: Debug + Send + Sync + 'static> BlockingQueueVec<E> {
  pub fn new() -> Self {
    Self {
      underlying: Arc::new(Mutex::new(QueueVec::new())),
      take: Arc::new(Mutex::new(MutexCvar::new(
        Mutex::new(false),
        Condvar::new(),
      ))),
      put: Arc::new(Mutex::new(MutexCvar::new(
        Mutex::new(false),
        Condvar::new(),
      ))),
    }
  }

  pub fn with_num_elements(num_elements: usize) -> Self {
    Self {
      underlying: Arc::new(Mutex::new(QueueVec::with_num_elements(num_elements))),
      take: Arc::new(Mutex::new(MutexCvar::new(
        Mutex::new(false),
        Condvar::new(),
      ))),
      put: Arc::new(Mutex::new(MutexCvar::new(
        Mutex::new(false),
        Condvar::new(),
      ))),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{env, thread};
  use std::sync::{Arc, Mutex};
  use std::thread::sleep;
  use std::time::Duration;

  use crate::{BlockingQueueBehavior, BlockingQueueVec};

  fn init_logger() {
    env::set_var("RUST_LOG", "debug");
    // env::set_var("RUST_LOG", "trace");
    let _ = logger::try_init();
  }

  #[test]
  fn test() {
    init_logger();
    let mut bqv1 = BlockingQueueVec::with_num_elements(1);
    let mut bqv2 = bqv1.clone();

    let handler = thread::spawn(move || {
      log::debug!("take: start: take");
      let n = bqv2.take();
      log::debug!("take: finish: take");
      log::debug!("take: n = {:?}", n);
    });

    log::debug!("put: start: sleep");
    sleep(Duration::from_secs(3));
    log::debug!("put: finish: sleep");

    log::debug!("put: start: put - 1");
    bqv1.put(1);
    log::debug!("put: finish: put - 1");

    log::debug!("put: start: put - 2");
    bqv1.put(2);
    log::debug!("put: finish: put - 2");

    handler.join().unwrap();
  }
}

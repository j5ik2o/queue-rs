mod blocking_queue_vec;
mod queue_vec;

pub use queue_vec::*;
pub use blocking_queue_vec::*;

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

pub trait BlockingQueueBehavior<E: Debug + Send + Sync>: QueueBehavior<E> {
  /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
  fn put(&mut self, e: E) -> Result<()>;
  /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
  fn take(&mut self) -> Option<E>;
}

#[cfg(test)]
extern crate env_logger;

#[cfg(test)]
mod tests {

  use std::{env, thread};
  use std::io::Write;
  use std::sync::{Arc, Mutex};
  use std::thread::sleep;
  use std::time::Duration;
  use fp_rust::sync::CountDownLatch;
  use crate::queue::{BlockingQueueVec, QueueBehavior};
  use crate::queue::BlockingQueueBehavior;

  fn init_logger() {
    env::set_var("RUST_LOG", "debug");
    // env::set_var("RUST_LOG", "trace");
    let _ = env_logger::try_init();
  }

  #[test]
  fn test() {
    init_logger();
    let cdl = CountDownLatch::new(1);
    let cdl2 = cdl.clone();

    let mut bqv1 = BlockingQueueVec::with_num_elements(64);
    let mut bqv2 = bqv1.clone();

    let max = 5;

    let handler1 = thread::spawn(move || {
      cdl2.countdown();
      for i in 1..=max {
        log::debug!("take: start: {}", i);
        let n = bqv2.take();
        log::debug!("take: finish: {},{:?}", i, n);
      }
    });

    cdl.wait();

    let handler2 = thread::spawn(move || {
      sleep(Duration::from_secs(3));

      for i in 1..=max {
        log::debug!("put: start: {}", i);
        //if i % 2 == 0 {
        //  bqv1.put(i).unwrap();
        //} else {
        bqv1.offer(i).unwrap();
        //}
        log::debug!("put: finish: {}", i);
      }
    });

    handler1.join().unwrap();
    handler2.join().unwrap();
  }
}

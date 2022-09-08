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

    pub fn with_elements(values: impl IntoIterator<Item=E> + ExactSizeIterator) -> Self {
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
    underlying: Arc<(Mutex<BlockingQueueVecInner<E>>, Condvar, Condvar)>,
}

#[derive(Debug, Clone)]
struct BlockingQueueVecInner<E> {
    queue: QueueVec<E>,
    count: usize,
}

impl<E> BlockingQueueVecInner<E> {
    fn new(queue: QueueVec<E>,
           count: usize) -> Self {
        Self {
            queue,
            count,
        }
    }
}

impl<E: Debug + Clone + Sync + Send + 'static> QueueBehavior<E> for BlockingQueueVec<E> {
    fn len(&self) -> usize {
        let (qg, _, _) = &*self.underlying;
        let lq = qg.lock().unwrap();
        lq.queue.len()
    }

    fn offer(&mut self, e: E) -> Result<()> {
        let (qg, _, _) = &*self.underlying;
        let mut lq = qg.lock().unwrap();
        lq.queue.offer(e)
    }

    fn poll(&mut self) -> Option<E> {
        let (qg, _, _) = &*self.underlying;
        let mut lq = qg.lock().unwrap();
        let result = lq.queue.poll();
        result
    }

    fn peek(&self) -> Option<E> {
        let (qg, _, _) = &*self.underlying;
        let lq = qg.lock().unwrap();
        lq.queue.peek()
    }
}

impl<E: Debug + Clone + Sync + Send + 'static> BlockingQueueBehavior<E> for BlockingQueueVec<E> {
    fn put(&mut self, e: E) -> Result<()> {
        let (qg, not_full, not_empty) = &*self.underlying;
        let mut lq = qg.lock().unwrap();
        while lq.count == lq.queue.num_elements {
            lq = not_full.wait(lq).unwrap();
        }
        let result = lq.queue.offer(e);
        lq.count += 1;
        not_empty.notify_all();
        result
    }

    fn take(&mut self) -> Option<E> {
        let (qg, not_full, not_empty) = &*self.underlying;
        let mut lq = qg.lock().unwrap();
        while lq.count == 0 {
            lq = not_empty.wait(lq).unwrap();
        }
        let result = lq.queue.poll();
        lq.count -= 1;
        not_full.notify_all();
        result
    }
}

impl<E: Debug + Send + Sync + 'static> BlockingQueueVec<E> {
    pub fn new() -> Self {
        Self {
            underlying: Arc::new(
                (Mutex::new(BlockingQueueVecInner::new(QueueVec::with_num_elements(32), 0)),
                 Condvar::new(), Condvar::new()
                )
            ),
        }
    }

    pub fn with_num_elements(num_elements: usize) -> Self {
        Self {
            underlying: Arc::new(
                (Mutex::new(BlockingQueueVecInner::new(QueueVec::with_num_elements(num_elements), 0)),
                 Condvar::new(), Condvar::new()
                )
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test0() {
        use std::sync::{Arc, Mutex, Condvar};
        use std::thread;

        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair2 = Arc::clone(&pair);

// Inside of our lock, spawn a new thread, and then wait for it to start.
        thread::spawn(move || {
            let (lock, cvar) = &*pair2;
            let mut started = lock.lock().unwrap();
            *started = true;
            // We notify the condvar that the value has changed.
            cvar.notify_one();
        });

// Wait for the thread to start up.
        let (lock, cvar) = &*pair;
        let mut started = lock.lock().unwrap();
        while !*started {
            started = cvar.wait(started).unwrap();
        }
    }

    use std::{env, thread};
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::thread::sleep;
    use std::time::Duration;
    use fp_rust::sync::CountDownLatch;

    use crate::{BlockingQueueBehavior, BlockingQueueVec};

    fn init_logger() {
        env::set_var("RUST_LOG", "debug");
        // env::set_var("RUST_LOG", "trace");
        let _ = logger::try_init();
    }

    #[test]
    fn test() {
        init_logger();
        let cdl = CountDownLatch::new(1);
        let cdl2 = cdl.clone();

        let mut bqv1 = BlockingQueueVec::with_num_elements(2);
        let mut bqv2 = bqv1.clone();

        let max = 5;

        let handler1 = thread::spawn(move || {
            cdl2.countdown();
            for i in 1..max {
                log::debug!("take: start: {}", i);
                let n = bqv2.take();
                log::debug!("take: finish: {},{:?}", i, n);
            }
        });

        cdl.wait();

        let handler2 = thread::spawn(move || {

            sleep(Duration::from_secs(3));

            for i in 1..max {
                log::debug!("put: start: {}", i);
                let _ = bqv1.put(i);
                log::debug!("put: finish: {}", i);
            }
        });

        handler1.join().unwrap();
        handler2.join().unwrap();
    }
}

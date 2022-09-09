mod blocking_queue_vec;
mod queue_mpsc;
mod queue_vec;

use std::cmp::Ordering;
pub use queue_vec::*;
pub use blocking_queue_vec::*;
pub use queue_mpsc::*;

use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use thiserror::Error;

pub trait Element: Debug + Clone + Send + Sync
{}

impl Element for i8 {}

impl Element for i16 {}

impl Element for i32 {}

impl Element for i64 {}

impl Element for u8 {}

impl Element for u16 {}

impl Element for u32 {}

impl Element for u64 {}

impl Element for usize {}

impl Element for f32 {}

impl Element for f64 {}

impl Element for String {}

impl<T: Debug + Clone + Send + Sync> Element for Box<T> {}

impl<T: Debug + Clone + Send + Sync> Element for Arc<T> {}

#[derive(Error, Debug)]
pub enum QueueError<E: Element> {
    #[error("Failed to offer an element: {0:?}")]
    OfferError(E),
    #[error("Failed to pool an element")]
    PoolError,
    #[error("Failed to peek an element")]
    PeekError,
}

#[derive(Debug, Clone)]
pub enum QueueSize {
    Limitless,
    Limited(usize),
}

impl QueueSize {
    fn increment(&mut self) {
        match self {
            QueueSize::Limited(c) => {
                *c += 1;
            }
            _ => {}
        }
    }
    fn decrement(&mut self) {
        match self {
            QueueSize::Limited(c) => {
                *c -= 1;
            }
            _ => {}
        }
    }
}

impl PartialEq<Self> for QueueSize {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (QueueSize::Limitless, QueueSize::Limitless) => true,
            (QueueSize::Limited(l), QueueSize::Limited(r)) => l == r,
            _ => false,
        }
    }
}

impl PartialOrd<QueueSize> for QueueSize {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (QueueSize::Limitless, QueueSize::Limitless) => Some(Ordering::Equal),
            (QueueSize::Limitless, _) => Some(Ordering::Greater),
            (_, QueueSize::Limitless) => Some(Ordering::Less),
            (QueueSize::Limited(l), QueueSize::Limited(r)) => l.partial_cmp(r),
        }
    }
}

pub trait QueueBehavior<E> {
    /// Returns whether this queue is empty.<br/>
    /// このキューが空かどうかを返します。
    fn is_empty(&self) -> bool {
        self.len() == QueueSize::Limited(0)
    }

    /// Returns whether this queue is non-empty.<br/>
    /// このキューが空でないかどうかを返します。
    fn non_empty(&self) -> bool {
        !self.is_empty()
    }

    /// Returns whether the queue size has reached its capacity.<br/>
    /// このキューのサイズが容量まで到達したかどうかを返します。
    fn is_full(&self) -> bool {
        self.capacity() == self.len()
    }

    /// Returns whether the queue size has not reached its capacity.<br/>
    /// このキューのサイズが容量まで到達してないかどうかを返します。
    fn non_full(&self) -> bool {
        !self.is_full()
    }

    /// Returns the length of this queue.<br/>
    /// このキューの長さを返します。
    fn len(&self) -> QueueSize;

    /// Returns the capacity of this queue.<br/>
    /// このキューの最大容量を返します。
    fn capacity(&self) -> QueueSize;

    /// The specified element will be inserted into this queue,
    /// if the queue can be executed immediately without violating the capacity limit.<br/>
    /// 容量制限に違反せずにすぐ実行できる場合は、指定された要素をこのキューに挿入します。
    fn offer(&mut self, e: E) -> Result<()>;

    /// Retrieves and deletes the head of the queue. Returns None if the queue is empty.<br/>
    /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
    fn poll(&mut self) -> Result<Option<E>>;
}

pub trait HasPeekBehavior<E: Element>: QueueBehavior<E> {
    /// Gets the head of the queue, but does not delete it. Returns None if the queue is empty.<br/>
    /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
    fn peek(&self) -> Result<Option<E>>;
}

pub trait BlockingQueueBehavior<E: Element>: QueueBehavior<E> {
    /// Inserts the specified element into this queue. If necessary, waits until space is available.<br/>
    /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
    fn put(&mut self, e: E) -> Result<()>;

    /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.<br/>
    /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
    fn take(&mut self) -> Result<Option<E>>;
}

pub enum QueueType {
    Vec,
    MPSC,
}

pub fn with_blocking<T: Element + 'static, Q: QueueBehavior<T>>(queue: Q) -> Box<dyn BlockingQueueBehavior<T>> {
    Box::new(BlockingQueue::new(queue))
}

pub fn create<T: Element + 'static>(queue_type: QueueType) -> Box<dyn QueueBehavior<T>> {
    match queue_type {
        QueueType::Vec => {
            Box::new(QueueVec::<T>::new())
        }
        QueueType::MPSC => {
            Box::new(QueueMPSC::<T>::new())
        }
    }
}

pub fn create_with_num_elements<T: Element + 'static>(
    queue_type: QueueType,
    blocking: bool,
    num_elements: usize,
) -> Box<dyn QueueBehavior<T>> {
    match queue_type {
        QueueType::Vec => {
            if blocking {
                Box::new(BlockingQueue::new(QueueVec::<T>::with_num_elements(num_elements)))
            } else {
                Box::new(QueueVec::<T>::with_num_elements(num_elements))
            }
        }
        QueueType::MPSC => {
            if blocking {
                Box::new(BlockingQueue::new(QueueMPSC::<T>::with_num_elements(num_elements)))
            } else {
                Box::new(QueueMPSC::<T>::with_num_elements(num_elements))
            }
        }
    }
}


#[cfg(test)]
extern crate env_logger;

#[cfg(test)]
mod tests {
    use std::{env, thread};
    use std::fmt::Debug;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::thread::sleep;
    use std::time::Duration;
    use fp_rust::sync::CountDownLatch;
    use crate::queue::{BlockingQueue, create, QueueBehavior, QueueMPSC, QueueType, QueueVec, with_blocking};
    use crate::queue::BlockingQueueBehavior;

    fn init_logger() {
        env::set_var("RUST_LOG", "debug");
        // env::set_var("RUST_LOG", "trace");
        let _ = env_logger::try_init();
    }

    fn test_blocking_queue_vec<Q>(queue: Q)
        where
            Q: BlockingQueueBehavior<i32> + Clone + Send + 'static,
    {
        let cdl = CountDownLatch::new(1);
        let cdl2 = cdl.clone();

        let mut bqv1 = queue;
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

    #[test]
    fn test() {
        init_logger();

        let q1 = create(QueueType::Vec);
        let q2 = with_blocking(*q1);

        // let inner_queue = QueueVec::<i32>::with_num_elements(5);
        test_blocking_queue_vec(*q2);

        // let inner_queue = QueueMPSC::<i32>::with_num_elements(5);
        // test_blocking_queue_vec(inner_queue);
    }

    #[derive(Debug)]
    struct Value<T: Debug>(T);

    fn println_n<T: Debug>(n: Value<T>) {
        println!("{:?}", n);
    }

    #[test]
    fn test1() {
        let n = Box::new(Value(1));
        println_n(*n);
    }
}

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Stream;
use tokio::sync::Mutex;
use tokio_condvar::Condvar;

use crate::queue::tokio::{BlockingQueueBehavior, HasContainsBehavior, HasPeekBehavior, QueueBehavior, QueueIter};
use crate::queue::{Element, QueueError, QueueSize};

#[derive(Clone)]
pub struct BlockingQueue<E: Element, Q: QueueBehavior<E>> {
  underlying: Arc<(Mutex<Q>, Condvar, Condvar)>,
  p: PhantomData<E>,
  is_interrupted: Arc<AtomicBool>,
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

  pub fn iter(self) -> QueueIter<E, BlockingQueue<E, Q>> {
    QueueIter {
      q: self,
      current_future: None,
      p: PhantomData,
    }
  }

  pub fn blocking_iter(self) -> BlockingQueueIter<E, Q> {
    BlockingQueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static, Q: QueueBehavior<E>> QueueBehavior<E> for BlockingQueue<E, Q> {
  async fn len(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    queue_vec_mutex_guard.len().await
  }

  async fn capacity(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    queue_vec_mutex_guard.capacity().await
  }

  async fn offer(&mut self, element: E) -> anyhow::Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let result = queue_vec_mutex_guard.offer(element).await;
    not_empty.notify_one();
    result
  }

  async fn offer_all(&mut self, elements: impl IntoIterator<Item = E> + Send) -> anyhow::Result<()> {
    let (queue_vec_mutex, _, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let result = queue_vec_mutex_guard.offer_all(elements).await;
    not_empty.notify_one();
    result
  }

  async fn poll(&mut self) -> anyhow::Result<Option<E>> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let result = queue_vec_mutex_guard.poll().await;
    not_full.notify_one();
    result
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static, Q: QueueBehavior<E> + HasPeekBehavior<E>> HasPeekBehavior<E> for BlockingQueue<E, Q> {
  async fn peek(&self) -> anyhow::Result<Option<E>> {
    let (queue_vec_mutex, not_full, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let result = queue_vec_mutex_guard.peek().await;
    not_full.notify_one();
    result
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static, Q: QueueBehavior<E> + HasContainsBehavior<E>> HasContainsBehavior<E>
  for BlockingQueue<E, Q>
{
  async fn contains(&self, element: &E) -> bool {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let result = queue_vec_mutex_guard.contains(element).await;
    result
  }
}

#[async_trait::async_trait]
impl<E: Element + 'static, Q: QueueBehavior<E>> BlockingQueueBehavior<E> for BlockingQueue<E, Q> {
  async fn put(&mut self, element: E) -> anyhow::Result<()> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    while queue_vec_mutex_guard.is_full().await {
      if self.check_and_update_interrupted() {
        log::debug!("put: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("put: blocking start...");
      queue_vec_mutex_guard = not_full.wait(queue_vec_mutex_guard).await;
      log::debug!("put: blocking end...");
    }
    let result = queue_vec_mutex_guard.offer(element).await;
    not_empty.notify_one();
    result
  }

  async fn take(&mut self) -> anyhow::Result<Option<E>> {
    let (queue_vec_mutex, not_full, not_empty) = &*self.underlying;
    let mut queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    while queue_vec_mutex_guard.is_empty().await {
      if self.check_and_update_interrupted() {
        log::debug!("take: return by interrupted");
        return Err(QueueError::<E>::InterruptedError.into());
      }
      log::debug!("take: blocking start...");
      queue_vec_mutex_guard = not_empty.wait(queue_vec_mutex_guard).await;
      log::debug!("take: blocking end...");
    }
    let result = queue_vec_mutex_guard.poll().await;
    not_full.notify_one();
    result
  }

  async fn remaining_capacity(&self) -> QueueSize {
    let (queue_vec_mutex, _, _) = &*self.underlying;
    let queue_vec_mutex_guard = queue_vec_mutex.lock().await;
    let capacity = queue_vec_mutex_guard.capacity().await;
    let len = queue_vec_mutex_guard.len().await;
    match (capacity.clone(), len.clone()) {
      (QueueSize::Limited(capacity), QueueSize::Limited(len)) => QueueSize::Limited(capacity - len),
      (QueueSize::Limitless, _) => QueueSize::Limitless,
      (_, _) => QueueSize::Limited(0),
    }
  }

  async fn interrupt(&mut self) {
    log::debug!("interrupt: start...");
    self.is_interrupted.store(true, Ordering::Relaxed);
    let (_, not_full, not_empty) = &*self.underlying;
    not_empty.notify_all();
    not_full.notify_all();
    log::debug!("interrupt: end...");
  }

  async fn is_interrupted(&self) -> bool {
    self.is_interrupted.load(Ordering::Relaxed)
  }
}

pub struct BlockingQueueIter<E: Element + 'static, Q: QueueBehavior<E>> {
  q: BlockingQueue<E, Q>,
  p: PhantomData<E>,
}

impl<E: Element + Unpin + 'static, Q: QueueBehavior<E>> Stream for BlockingQueueIter<E, Q> {
  type Item = E;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let bq = &mut self.get_mut().q;
    match Pin::new(&mut bq.take()).poll(cx) {
      Poll::Ready(Ok(Some(value))) => Poll::Ready(Some(value)),
      Poll::Ready(Ok(None)) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
      _ => {
        panic!("BlockingQueueIter: poll_next: unexpected error");
      }
    }
  }
}

use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::queue::{Element, QueueBehavior, QueueError, QueueIntoIter, QueueIter, QueueSize};

/// A queue implementation backed by a `MPSC`.<br/>
/// `QueueMPSC` で実装されたキュー。
#[derive(Debug, Clone)]
pub struct QueueMPSC<E> {
  inner: Arc<Mutex<QueueMPSCInner<E>>>,
  tx: Sender<E>,
}

#[derive(Debug)]
struct QueueMPSCInner<E> {
  rx: Receiver<E>,
  count: QueueSize,
  capacity: QueueSize,
}

impl<E: Element + 'static> QueueBehavior<E> for QueueMPSC<E> {
  fn len(&self) -> QueueSize {
    let inner_guard = self.inner.lock().unwrap();
    inner_guard.count.clone()
  }

  fn capacity(&self) -> QueueSize {
    let inner_guard = self.inner.lock().unwrap();
    inner_guard.capacity.clone()
  }

  fn offer(&mut self, element: E) -> Result<()> {
    match self.tx.send(element) {
      Ok(_) => {
        let mut inner_guard = self.inner.lock().unwrap();
        inner_guard.count.increment();
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err).into()),
    }
  }

  fn poll(&mut self) -> Result<Option<E>> {
    let mut inner_guard = self.inner.lock().unwrap();
    match inner_guard.rx.try_recv() {
      Ok(element) => {
        inner_guard.count.decrement();
        Ok(Some(element))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(QueueError::<E>::PoolError.into()),
    }
  }
}

impl<E: Element + 'static> QueueMPSC<E> {
  /// Create a new `QueueMPSC`.<br/>
  /// 新しい `QueueMPSC` を作成します。
  pub fn new() -> Self {
    let (tx, rx) = channel();
    Self {
      inner: Arc::new(Mutex::new(QueueMPSCInner {
        rx,
        count: QueueSize::Limited(0),
        capacity: QueueSize::Limitless,
      })),
      tx,
    }
  }

  /// Update the maximum number of elements in the queue.<br/>
  /// キューの最大要素数を更新します。
  ///
  /// # Arguments / 引数
  /// - `capacity` - The maximum number of elements in the queue. / キューの最大要素数。
  pub fn with_capacity(self, capacity: QueueSize) -> Self {
    {
      let mut inner_guard = self.inner.lock().unwrap();
      inner_guard.capacity = capacity;
    }
    self
  }

  /// Update the elements in the queue.<br/>
  /// キューの要素を更新します。
  ///
  /// # Arguments / 引数
  /// - `elements` - The elements to be updated. / 更新する要素。
  pub fn with_elements(mut self, elements: impl IntoIterator<Item = E> + ExactSizeIterator) -> Self {
    self.offer_all(elements).unwrap();
    self
  }

  pub fn iter(&mut self) -> QueueIter<E, QueueMPSC<E>> {
    QueueIter {
      q: self,
      p: PhantomData,
    }
  }
}

impl<E: Element + 'static> IntoIterator for QueueMPSC<E> {
  type IntoIter = QueueIntoIter<E, QueueMPSC<E>>;
  type Item = E;

  fn into_iter(self) -> Self::IntoIter {
    QueueIntoIter {
      q: self,
      p: PhantomData,
    }
  }
}

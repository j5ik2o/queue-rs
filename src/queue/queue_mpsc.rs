use std::fmt::Debug;
use std::sync::mpsc::{channel, Receiver, Sender, SendError, TryRecvError};
use std::sync::{Arc, Mutex};
use crate::queue::{QueueBehavior, QueueError, QueueSize};

use anyhow::anyhow;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct QueueMPSC<E> {
  rx: Arc<Mutex<Receiver<E>>>,
  tx: Sender<E>,
}

impl<E: Debug + Clone + Send + Sync + 'static> QueueBehavior<E> for QueueMPSC<E> {
  fn len(&self) -> QueueSize {
    QueueSize::Limitless
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::Limitless
  }

  fn offer(&mut self, e: E) -> anyhow::Result<()> {
    match self.tx.send(e) {
      Ok(_) => Ok(()),
      Err(SendError(e)) => Err(anyhow::Error::new(QueueError::OfferError(e))),
    }
  }

  fn poll(&mut self) -> Result<Option<E>> {
    let receiver_guard = self.rx.lock().unwrap();
    match receiver_guard.try_recv() {
      Ok(e) => Ok(Some(e)),
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(anyhow::Error::new(QueueError::<E>::PoolError)),
    }
  }
}

impl<E: Debug + Clone + Send + Sync + 'static> QueueMPSC<E> {
  pub fn new() -> Self {
    let (tx, rx) = channel();
    Self {
      rx: Arc::new(Mutex::new(rx)),
      tx,
    }
  }
}

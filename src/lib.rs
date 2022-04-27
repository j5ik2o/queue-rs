use std::fmt::Debug;
use std::thread::sleep;

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
    fn offer(&mut self, e: E) -> Result<bool>;
    /// キューの先頭を取得および削除します。キューが空の場合は None を返します。
    fn poll(&mut self) -> Option<E>;
    /// キューの先頭を取得しますが、削除しません。キューが空の場合は None を返します。
    fn peek(&self) -> Option<&E>;
}

pub trait BlockingQueueBehavior<E: Debug + Send + Sync>: QueueBehavior<E> {
    /// 指定された要素をこのキューに挿入します。必要に応じて、空きが生じるまで待機します。
    fn put(&mut self, e: E) -> Result<()>;
    /// このキューの先頭を取得して削除します。必要に応じて、要素が利用可能になるまで待機します。
    fn take(&mut self) -> Option<E>;
}

#[cfg(test)]
mod tests {
    use std::env;

    fn init_logger() {
        env::set_var("RUST_LOG", "debug");
        // env::set_var("RUST_LOG", "trace");
        let _ = logger::try_init();
    }
}

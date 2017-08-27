
extern crate spmc;
extern crate itertools;
extern crate rand;

use std::time::Duration;

pub mod task;
mod worker;
pub mod pool;
mod boolchan;
mod taskchan;
mod channel;

#[derive(Clone, Copy)]
pub enum ShareStrategy {
    One,
    Half,
}

#[derive(Clone, Copy)]
pub enum Initiated {
    Sender,
    Receiver,
}

#[derive(Clone, Copy)]
pub enum ReceiverWaitStrategy {
    Yield,
    Sleep(Duration),
}

pub use task::Task;
pub use pool::ThreadPool;
pub use worker::Worker;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

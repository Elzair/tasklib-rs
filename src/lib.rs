
extern crate itertools;
extern crate rand;

use std::time::Duration;

pub mod task;
mod worker;
pub mod pool;
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
pub use pool::{PoolRI, PoolSI};
// pub use worker::{WorkerRI, WorkerSI};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

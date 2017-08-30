
extern crate itertools;
extern crate rand;

use std::time::Duration;

pub mod task;
pub mod worker;
pub mod work_share;

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
pub use worker::Worker;
// pub use pool::{PoolRI, PoolSI};
// pub use worker::{WorkerRI, WorkerSI};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}


extern crate itertools;
extern crate rand;

use std::time::Duration;

mod rng;
pub mod task;
pub mod worker;

pub mod work_share;
// pub mod work_take;

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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

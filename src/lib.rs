
extern crate spmc;
extern crate itertools;
extern crate rand;

pub mod task;
mod worker;
pub mod pool;
mod channel;

#[derive(Clone, Copy)]
pub enum ShareStrategy {
    ONE,
    HALF,
}

#[derive(Clone, Copy)]
pub enum Initiated {
    SENDER,
    RECEIVER,
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

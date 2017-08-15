
extern crate itertools;
extern crate rand;

pub mod task;
mod worker;
pub mod pool;

pub use task::Task;
pub use worker::ShareStrategy;
pub use pool::ThreadPool;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

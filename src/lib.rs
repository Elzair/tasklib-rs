
extern crate itertools;
extern crate rand;
extern crate reqchan;

mod rng;
pub mod task;
pub mod worker;

pub mod work_share;
pub mod work_take;

pub use task::Task;
pub use worker::Worker;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

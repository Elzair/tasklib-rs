use std::cell::RefCell;
use std::sync::Arc;

use rand;
use rand::{Rng, SeedableRng};

use super::super::rng;
use super::super::Worker as WorkerTrait;
use super::super::task::Task;
use super::shared::Data as SharedData;
use super::shared::TryGetTaskError;

pub struct Config {
    pub index: usize,
    pub shared_data: Arc<SharedData>,
}

pub struct Worker {
    index: usize,
    shared_data: Arc<SharedData>,
    rng: RefCell<rand::XorShiftRng>,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        Worker {
            index: config.index,
            shared_data: config.shared_data,
            rng: RefCell::new(rand::XorShiftRng::from_seed(rng::rand_seed())),
        }
    }

    pub fn run(&self) {
        while !self.shared_data.should_exit() {
            self.run_once();
        }

        // Run any remaining tasks if instructed to do so.
        if self.shared_data.should_run_tasks() {
            let mut is_empty = false;

            while !is_empty {
                match self.shared_data.try_get_task(self.index) {
                    Ok(task) => {
                        task.call_box();
                    },
                    Err(TryGetTaskError::Empty) => {
                        is_empty = true;
                    },
                    // Wait for "local" queue to be unlocked.
                    Err(TryGetTaskError::Locked) => {},
                }
            }
        }

        // Wait on all other workers to stop running before dropping everything.
        self.shared_data.wait_on_exit();
    }
    
    pub fn run_once(&self) {
        // First, try to execute a task from the "local" queue.
        if let Ok(task) = self.shared_data.try_get_task(self.index) {
            task.call_box();
            return;
        }

        // Then, resort to executing tasks from other queues.
        loop {
            let rand_index = self.rand_index();

            if let Ok(task) = self.shared_data.try_get_task(rand_index) {
                task.call_box();
                return;
            }
        }
    }

    #[inline]
    fn rand_index(&self) -> usize {
        self.rng.borrow_mut().gen::<usize>()
            % self.shared_data.queues.len()
    }
}

impl WorkerTrait for Worker {
    #[inline]
    fn get_index(&self) -> usize {
        self.index
    }

    #[inline]
    fn signal_exit(&self) {
        self.shared_data.signal_exit(false);
    }

    #[inline]
    fn add_task(&self, task: Task) {
        self.shared_data.add_task(self.index, task);
    }
}

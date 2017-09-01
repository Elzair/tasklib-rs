use std::collections::VecDeque;
use std::sync::{Barrier, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use super::super::task::Task;

pub struct Data {
    pub queues: Vec<Mutex<VecDeque<Task>>>,
    exit_flag: AtomicBool,
    exit_barrier: Barrier,
    run_all_tasks_before_exit: AtomicBool,
}

impl Data {
    pub fn new(n: usize, capacity: usize) -> Data {
        #[allow(unused_variables)]
        let queues = (0..n).into_iter()
            .map(|nn| {
                Mutex::new(VecDeque::<Task>::with_capacity(capacity))
            }).collect::<Vec<_>>();

        Data {
            queues: queues,
            exit_flag: AtomicBool::new(false),
            exit_barrier: Barrier::new(n),
            run_all_tasks_before_exit: AtomicBool::new(true),
        }
    }

    #[inline]
    pub fn should_exit(&self) -> bool {
        self.exit_flag.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn should_run_tasks(&self) -> bool {
        self.run_all_tasks_before_exit.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn signal_exit(&self, panicking: bool) {
        if panicking {
            // Tell workers to exit ASAP.
            self.run_all_tasks_before_exit.store(false, Ordering::SeqCst);
        }

        self.exit_flag.store(true, Ordering::SeqCst);
    }

    #[inline]
    pub fn wait_on_exit(&self) {
        self.exit_barrier.wait();
    }

    #[inline]
    pub fn add_task(&self, index: usize, task: Task) {
        let mut guard = self.queues[index].lock().unwrap();

        guard.push_back(task);
    }

    #[inline]
    pub fn try_get_task(&self, index: usize) -> Result<Task, TryGetTaskError> {
        match self.queues[index].try_lock() {
            Ok(mut guard) => {
                match guard.pop_front() {
                    Some(task) => Ok(task),
                    None => Err(TryGetTaskError::Empty),
                }
            },
            // Assume the Mutex is never poisoned.
            // Nothing should panic while this Mutex is held.
            _ => Err(TryGetTaskError::Locked),
        }
    }
}

pub enum TryGetTaskError {
    Locked,
    Empty,
}

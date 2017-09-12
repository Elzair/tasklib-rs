use std::collections::VecDeque;
use std::sync::{Barrier, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use super::super::Task;

pub struct Data {
    queues: Vec<Mutex<VecDeque<Box<Task>>>>,
    exit_flag: AtomicBool,
    exit_barrier: Barrier,
    run_all_tasks_before_exit: AtomicBool,
    num_queues: usize,
}

impl Data {
    pub fn new(n: usize, capacity: usize) -> Data {
        #[allow(unused_variables)]
        let queues = (0..n).into_iter()
            .map(|nn| {
                Mutex::new(VecDeque::<Box<Task>>::with_capacity(capacity))
            }).collect::<Vec<_>>();

        Data {
            queues: queues,
            exit_flag: AtomicBool::new(false),
            exit_barrier: Barrier::new(n),
            run_all_tasks_before_exit: AtomicBool::new(true),
            num_queues: n,
        }
    }

    #[inline]
    pub fn get_num_queues(&self) -> usize {
        self.num_queues
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
    pub fn add_task(&self, index: usize, task: Box<Task>) {
        let mut guard = self.queues[index].lock().unwrap();

        guard.push_back(task);
    }

    #[inline]
    pub fn try_get_task(&self, index: usize) -> Result<Box<Task>,
                                                       TryGetTaskError> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::thread;

    use super::*;

    #[test]
    fn test_make_shared() {
        #[allow(unused_variables)]
        let shared = Data::new(2, 16);

        assert_eq!(shared.queues.len(), 2);
        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);
    }

    #[test]
    fn test_should_exit() {
        let shared = Data::new(2, 16);

        assert_eq!(shared.should_exit(), false);

        shared.exit_flag.store(true, Ordering::SeqCst);

        assert_eq!(shared.should_exit(), true);
    }

    #[test]
    fn test_should_run_tasks() {
        let shared = Data::new(2, 16);

        assert_eq!(shared.should_run_tasks(), true);

        shared.run_all_tasks_before_exit.store(false, Ordering::SeqCst);

        assert_eq!(shared.should_run_tasks(), false);
    }

    #[test]
    fn test_signal_exit_no_panic() {
        let shared = Data::new(2, 16);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);

        shared.signal_exit(false);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), true);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);
    }

    #[test]
    fn test_signal_exit_panic() {
        let shared = Data::new(2, 16);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);

        shared.signal_exit(true);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), true);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), false);
    }

    #[test]
    fn test_wait_on_exit() {
        let shared = Arc::new(Data::new(2, 16));
        let shared2 = shared.clone();

        let handle = thread::spawn(move || {
            shared2.wait_on_exit();
        });

        shared.wait_on_exit();
        handle.join().unwrap();
    }
}

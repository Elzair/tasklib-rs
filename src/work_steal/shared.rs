use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam::sync::SegQueue;

use super::super::Task;

pub struct Data {
    queues: Vec<SegQueue<Box<Task>>>,
    exit_flag: AtomicBool,
    exit_barrier: Barrier,
    run_all_tasks_before_exit: AtomicBool,
    num_queues: usize,
}

impl Data {
    pub fn new(num_workers: usize) -> Data {
        #[allow(unused_variables)]
        let queues = (0..num_workers).into_iter()
            .map(|n| {
                SegQueue::<Box<Task>>::new()
            }).collect::<Vec<_>>();

        Data {
            queues: queues,
            exit_flag: AtomicBool::new(false),
            exit_barrier: Barrier::new(num_workers),
            run_all_tasks_before_exit: AtomicBool::new(true),
            num_queues: num_workers,
        }
    }

    #[inline]
    pub fn add_task(&self, index: usize, task: Box<Task>) {
        self.queues[index].push(task);
    }

    #[inline]
    pub fn try_get_task(&self, index: usize) -> Option<Box<Task>> {
        self.queues[index].try_pop()
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use super::*;

    #[test]
    fn test_make_shared() {
        #[allow(unused_variables)]
        let shared = Data::new(2);

        assert_eq!(shared.queues.len(), 2);
        assert_eq!(shared.queues[0].is_empty(), true);
        assert_eq!(shared.queues[1].is_empty(), true);
        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst),
                   true);
    }

    #[test]
    fn test_add_task() {
        let shared = Data::new(1);

        shared.add_task(0, Box::new(move || { println!("Hello world!"); }));

        assert_eq!(shared.queues[0].is_empty(), false);
    }

    #[test]
    fn test_try_get_task() {
        let shared = Data::new(1);

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        shared.queues[0].push(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }));

        match shared.try_get_task(0) {
            Some(task) => { task.call_box(); },
            None => { assert!(false); },
        }
        if let Some(task) = shared.try_get_task(0) {
            task.call_box();
        }

        match shared.try_get_task(0) {
            None => {},
            Some(_) => { assert!(false); },
        }

        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_should_exit() {
        let shared = Data::new(1);

        assert_eq!(shared.should_exit(), false);

        shared.exit_flag.store(true, Ordering::SeqCst);

        assert_eq!(shared.should_exit(), true);
    }

    #[test]
    fn test_should_run_tasks() {
        let shared = Data::new(1);

        assert_eq!(shared.should_run_tasks(), true);

        shared.run_all_tasks_before_exit.store(false, Ordering::SeqCst);

        assert_eq!(shared.should_run_tasks(), false);
    }

    #[test]
    fn test_signal_exit_no_panic() {
        let shared = Data::new(1);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);

        shared.signal_exit(false);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), true);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);
    }

    #[test]
    fn test_signal_exit_panic() {
        let shared = Data::new(1);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), false);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), true);

        shared.signal_exit(true);

        assert_eq!(shared.exit_flag.load(Ordering::SeqCst), true);
        assert_eq!(shared.run_all_tasks_before_exit.load(Ordering::SeqCst), false);
    }

    #[test]
    fn test_wait_on_exit() {
        let shared = Arc::new(Data::new(2));
        let shared2 = shared.clone();

        let handle = thread::spawn(move || {
            shared2.wait_on_exit();
        });

        shared.wait_on_exit();
        handle.join().unwrap();
    }
}

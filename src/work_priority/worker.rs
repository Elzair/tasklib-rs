use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand;
use rand::{Rng, SeedableRng};

use super::super::rng;
use super::super::Task;
use super::shared::Data as SharedData;

pub struct Config {
    pub index: usize,
    pub shared_data: Arc<SharedData>,
    pub timeout: Duration,
}

pub struct Worker {
    index: usize,
    shared_data: Arc<SharedData>,
    timeout: Duration,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        Worker {
            index: config.index,
            shared_data: config.shared_data,
            timeout: config.timeout,
        }
    }

    pub fn add_task(&self, priority: usize, task: Box<Task>) {
        self.shared_data.add_task(priority, task);
    }

    pub fn run(&self) {
        while !self.shared_data.should_exit() {
            self.run_once();
        }

        // Run any remaining tasks if instructed to do so.
        if self.shared_data.should_run_tasks() {
            while let Some(task) = self.shared_data.try_get_task() {
                task.call_box();
            }
        }

        // Wait on all other workers to stop running before dropping everything.
        self.shared_data.wait_on_exit();
    }
    
    pub fn run_once(&self) {
        // Loop trying to find tasks until getting one or timing out.
        let start_time = Instant::now();
        
        loop {
            if let Some(task) = self.shared_data.try_get_task() {
                task.call_box();
                return;
            }
            
            if Instant::now().duration_since(start_time) >= self.timeout {
                return;
            }
        }
    }

    #[inline]
    fn signal_exit(&self) {
        self.shared_data.signal_exit(false);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;
    
    use super::*;

    fn helper(num_queues: usize,
              timeout: Duration) -> (Worker, Worker) {
        let shared_data = Arc::new(SharedData::new(num_queues));

        let worker1 = Worker::new(Config {
            index: 0,
            shared_data: shared_data.clone(),
            timeout: timeout,
        });

        let worker2 = Worker::new(Config {
            index: 1,
            shared_data: shared_data.clone(),
            timeout: timeout,
        });

        (worker1, worker2)
    }

    #[test]
    fn test_make_worker() {
        let shared_data = Arc::new(SharedData::new(1));

        #[allow(unused_variables)]
        let worker = Worker::new(Config {
            index: 0,
            shared_data: shared_data,
            timeout: Duration::new(0, 100),
        });
    }

    #[test]
    fn test_worker_run_once() {
        let (worker1, worker2) = helper(1, Duration::new(1, 0));

        let var = Arc::new(AtomicUsize::new(0));
        let var2 = var.clone();

        worker1.add_task(0, Box::new(move || {
            var2.fetch_add(1, Ordering::SeqCst);
        }));

        worker1.run_once();

        assert_eq!(var.load(Ordering::SeqCst), 1);

        // This should time out.
        worker2.run_once();
    }

    #[test]
    fn test_worker_run_signal_exit() {
        let (worker1, worker2) = helper(1, Duration::new(0, 200));

        let handle = thread::spawn(move || {
            worker2.run();
        });
        
        thread::sleep(Duration::new(0, 500));
        worker1.signal_exit();
        worker1.shared_data.wait_on_exit();

        handle.join().unwrap();
    }
}

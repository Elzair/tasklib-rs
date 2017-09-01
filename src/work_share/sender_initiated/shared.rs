use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Data {
    exit_flag: AtomicBool,
    exit_barrier: Barrier,
    run_all_tasks_before_exit: AtomicBool,
}

impl Data {
    pub fn new(n: usize) -> Data {
        Data {
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
}

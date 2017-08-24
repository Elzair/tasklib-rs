use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct SpinLock {
    spin_lock: AtomicBool,
}

impl SpinLock {
    pub fn new() -> SpinLock {
        SpinLock {
            spin_lock: AtomicBool::new(false),
        }
    }

    pub fn lock(&self, timeout: Option<Duration>) -> bool {
        let instant = Instant::now();
        
        loop {
            let is_locked = self.try_lock();

            if is_locked == true {
                return true;
            }

            match timeout {
                Some(duration) => {
                    if Instant::now().duration_since(instant) >= duration {
                        return false;
                    }
                },
                None => {},
            }
        }
    }

    pub fn try_lock(&self) -> bool {
        let (current, new) = (false, true);
        
        let res = self.spin_lock.compare_and_swap(current,
                                                  new,
                                                  Ordering::Acquire);
        
        res == current
    }

    pub fn unlock(&self) -> bool {
        let (current, new) = (true, false);

        let res = self.spin_lock.compare_and_swap(current,
                                                  new,
                                                  Ordering::Release);

        res == current
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use super::SpinLock;

    #[test]
    fn test_spinlock_new() {
        let slock = SpinLock::new();

        assert!(slock.spin_lock.load(Ordering::Relaxed) == false);
    }

    #[test]
    fn test_spinlock_try_lock() {
        let slock = SpinLock::new();
        let res1 = slock.try_lock();

        assert!(res1 == true);
        assert!(slock.spin_lock.load(Ordering::Relaxed) == true);

        let res2 = slock.try_lock();

        assert!(res2 == false);
        assert!(slock.spin_lock.load(Ordering::Relaxed) == true);
    }

    #[test]
    fn test_spinlock_lock() {
        let slock = SpinLock::new();
        let res1 = slock.lock(None);

        assert!(res1 == true);
        assert!(slock.spin_lock.load(Ordering::Relaxed) == true);

        let res2 = slock.lock(Some(Duration::new(1, 0)));

        assert!(res2 == false);
        assert!(slock.spin_lock.load(Ordering::Relaxed) == true);
    }

    #[test]
    fn test_spinlock_unlock() {
        let slock = SpinLock::new();
        slock.lock(None);

        let res2 = slock.unlock();

        assert!(res2 == true);
        assert!(slock.spin_lock.load(Ordering::Relaxed) == false);
    }
}

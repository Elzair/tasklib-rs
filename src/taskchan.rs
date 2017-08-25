use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use std::thread;

use super::ReceiverWaitStrategy as WaitStrategy;
use task::Task;

pub enum Data {
    NoTasks,
    OneTask(Task),
    ManyTasks(VecDeque<Task>),
}

pub fn make_taskchan(
    rx_wait: WaitStrategy,
    rx_timeout: Duration,
) -> (Sender, Receiver) {
    let inner: Arc<Mutex<Option<Data>>> = Arc::new(Mutex::new(None));
    
    (
        Sender { inner: inner.clone() },
        Receiver {
            inner: inner.clone(),
            wait_strategy: rx_wait,
            timeout: rx_timeout,
        },
    )
}

pub struct Sender {
    inner: Arc<Mutex<Option<Data>>>,
}

impl Sender {
    pub fn send(&self, data: Data) -> Result<(), SendError> {
        let mut value = self.inner.lock().unwrap();

        match *value {
            None => {},
            _ => { return Err(SendError::UnreceivedValue); },
        }

        *value = Some(data);

        Ok(())
    }
}

pub struct Receiver {
    inner: Arc<Mutex<Option<Data>>>,
    wait_strategy: WaitStrategy,
    timeout: Duration,
}

impl Receiver {
    pub fn try_receive(&self) -> Result<Data, TryReceiveError> {
        let mut value = self.inner.lock().unwrap();

        match (*value).take() {
            Some(x) => Ok(x),
            None => Err(TryReceiveError::Empty),
        }
    }

    pub fn receive(&self) -> Result<Data, ReceiveError> {
        let start_time = Instant::now();
        
        loop {
            if let Ok(data) = self.try_receive() {
                return Ok(data);
            }
            else {
                match self.wait_strategy {
                    WaitStrategy::Yield => {
                        thread::yield_now();
                    },
                    WaitStrategy::Sleep(duration) => {
                        thread::sleep(duration);
                    },
                };
            }

            if Instant::now().duration_since(start_time) >= self.timeout {
                return Err(ReceiveError::Timeout);
            }
        }
    }
}

#[derive(Debug)]
pub enum SendError {
    UnreceivedValue,
}

#[derive(Debug)]
pub enum TryReceiveError {
    Empty,
}

#[derive(Debug)]
pub enum ReceiveError {
    Timeout,
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    
    use super::{Data, Sender, Receiver, SendError, TryReceiveError, ReceiveError, make_taskchan};
    use super::super::ReceiverWaitStrategy;
    use super::super::task::Task;

    fn helper() -> (Sender, Receiver) {
        make_taskchan(
            ReceiverWaitStrategy::Sleep(Duration::new(0, 10000)),
            Duration::new(0, 1000000)
        )
    }

    #[test]
    fn test_creation() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();
    }

    #[test]
    fn test_sender_send_no_tasks() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        tx.send(Data::NoTasks).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(Data::NoTasks) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_sender_send_one_task() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        let data = Data::OneTask(Box::new(|| { println!("Hello world!"); }));
        tx.send(data).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(Data::OneTask(_)) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_sender_send_many_tasks() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        let data1 = Box::new(|| { println!("Hello world!"); });
        let data2 = Box::new(|| { println!("Hello again!"); });
        let mut data = VecDeque::<Task>::new();
        data.push_back(data1);
        data.push_back(data2);

        tx.send(Data::ManyTasks(data)).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(Data::ManyTasks(_)) => {},
            _ => { assert!(false); },
        };
    }
    
    #[test]
    fn test_sender_send_unreceived() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        tx.send(Data::NoTasks).unwrap();

        match tx.send(Data::NoTasks) {
            Err(SendError::UnreceivedValue) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_try_receive_no_tasks() {
        let (tx, rx) = helper();

        tx.send(Data::NoTasks).unwrap();

        match rx.try_receive() {
            Ok(Data::NoTasks) => {},
            _ => { assert!(false); },
        };

        match *rx.inner.lock().unwrap() {
            None => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_try_receive_one_task() {
        let (tx, rx) = helper();

        let var = Arc::new(AtomicUsize::new(0));
        let var2 = var.clone();

        let data = Data::OneTask(Box::new(move || {
            var2.fetch_add(1, Ordering::SeqCst);
        }));

        tx.send(data).unwrap();

        match rx.try_receive() {
            Ok(Data::OneTask(task)) => {
                task.call_box();
            },
            _ => { assert!(false); },
        };

        assert_eq!(var.load(Ordering::SeqCst), 1);

        match *rx.inner.lock().unwrap() {
            None => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_try_receive_many_tasks() {
        let (tx, rx) = helper();

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();

        let task1 = Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }) as Task;
        let task2 = Box::new(move || {
            var2.fetch_add(1, Ordering::SeqCst);
        }) as Task;

        let mut vd = VecDeque::new();
        vd.push_back(task1);
        vd.push_back(task2);

        let data = Data::ManyTasks(vd);

        tx.send(data).unwrap();

        match rx.try_receive() {
            Ok(Data::ManyTasks(tasks)) => {
                for task in tasks.into_iter() {
                    task.call_box();
                }
            },
            _ => { assert!(false); },
        };

        assert_eq!(var.load(Ordering::SeqCst), 2);

        match *rx.inner.lock().unwrap() {
            None => {},
            _ => { assert!(false); },
        };
    }
    
    #[test]
    fn test_receiver_try_receive_empty() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        match rx.try_receive() {
            Err(TryReceiveError::Empty) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_receive() {
        let (tx, rx) = helper();

        tx.send(Data::NoTasks).unwrap();

        match rx.receive() {
            Ok(Data::NoTasks) => {},
            _ => { assert!(false); },
        };

        match *rx.inner.lock().unwrap() {
            None => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_receive_timeout() {
        #[allow(unused_variables)]
        let (tx, rx) = helper();

        match rx.receive() {
            Err(ReceiveError::Timeout) => {},
            _ => { assert!(false); },
        };
    }
}

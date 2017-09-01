use std::sync::{Arc, Mutex};

use super::super::task::Data as TaskData;

pub fn channel() -> (Sender, Receiver) {
    let inner: Arc<Mutex<Option<TaskData>>> = Arc::new(Mutex::new(None));
    
    (
        Sender { inner: inner.clone() },
        Receiver {
            inner: inner.clone(),
        },
    )
}

pub struct Sender {
    inner: Arc<Mutex<Option<TaskData>>>,
}

impl Sender {
    pub fn send(&self, data: TaskData) -> Result<(), SendError> {
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
    inner: Arc<Mutex<Option<TaskData>>>,
}

impl Receiver {
    #[inline]
    pub fn try_receive(&self) -> Result<TaskData, TryReceiveError> {
        let mut value = self.inner.lock().unwrap();

        match (*value).take() {
            Some(x) => Ok(x),
            None => Err(TryReceiveError::Empty),
        }
    }

    pub fn receive(&self) -> TaskData {
        loop {
            if let Ok(data) = self.try_receive() {
                return data;
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    use super::*;
    use super::super::super::super::task::Task;

    #[test]
    fn test_creation() {
        #[allow(unused_variables)]
        let (tx, rx) = channel();
    }

    #[test]
    fn test_sender_send_no_tasks() {
        #[allow(unused_variables)]
        let (tx, rx) = channel();

        tx.send(TaskData::NoTasks).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(TaskData::NoTasks) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_sender_send_one_task() {
        #[allow(unused_variables)]
        let (tx, rx) = channel();

        let data = TaskData::OneTask(Box::new(|| { println!("Hello world!"); }));
        tx.send(data).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(TaskData::OneTask(_)) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_sender_send_many_tasks() {
        #[allow(unused_variables)]
        let (tx, rx) = channel();

        let data1 = Box::new(|| { println!("Hello world!"); });
        let data2 = Box::new(|| { println!("Hello again!"); });
        let mut data = VecDeque::<Task>::new();
        data.push_back(data1);
        data.push_back(data2);

        tx.send(TaskData::ManyTasks(data)).unwrap();

        match *tx.inner.lock().unwrap() {
            Some(TaskData::ManyTasks(_)) => {},
            _ => { assert!(false); },
        };
    }
    
    #[test]
    fn test_sender_send_unreceived() {
        #[allow(unused_variables)]
        let (tx, rx) = channel();

        tx.send(TaskData::NoTasks).unwrap();

        match tx.send(TaskData::NoTasks) {
            Err(SendError::UnreceivedValue) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_try_receive_no_tasks() {
        let (tx, rx) = channel();

        tx.send(TaskData::NoTasks).unwrap();

        match rx.try_receive() {
            Ok(TaskData::NoTasks) => {},
            _ => { assert!(false); },
        };

        match *rx.inner.lock().unwrap() {
            None => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_try_receive_one_task() {
        let (tx, rx) = channel();

        let var = Arc::new(AtomicUsize::new(0));
        let var2 = var.clone();

        let data = TaskData::OneTask(Box::new(move || {
            var2.fetch_add(1, Ordering::SeqCst);
        }));

        tx.send(data).unwrap();

        match rx.try_receive() {
            Ok(TaskData::OneTask(task)) => {
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
        let (tx, rx) = channel();

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

        let data = TaskData::ManyTasks(vd);

        tx.send(data).unwrap();

        match rx.try_receive() {
            Ok(TaskData::ManyTasks(tasks)) => {
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
        let (tx, rx) = channel();

        match rx.try_receive() {
            Err(TryReceiveError::Empty) => {},
            _ => { assert!(false); },
        };
    }

    #[test]
    fn test_receiver_receive() {
        let (tx, rx) = channel();

        tx.send(TaskData::NoTasks).unwrap();

        #[allow(unused_variables)]
        match rx.receive() {
            TaskData::NoTasks => {},
            _ => { assert!(false); },
        };
    }
}

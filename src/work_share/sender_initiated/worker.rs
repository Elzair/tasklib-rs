use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rand;
use rand::{Rng, SeedableRng};
use reqchan::{Requester, Responder, TryReceiveError, TryRespondError};

use super::super::super::rng;
use super::super::super::Worker as WorkerTrait;
use super::super::super::task::Task;
use super::super::{ShareStrategy, ReceiverWaitStrategy, TaskData};
use super::shared::Data as SharedData;

pub struct Config {
    pub index: usize,
    pub shared_data: Arc<SharedData>,
    pub task_capacity: usize,
    pub share_strategy: ShareStrategy,
    pub wait_strategy: ReceiverWaitStrategy,
    pub receiver_timeout: Duration,
    pub requester: Requester<TaskData>,
    pub responders: Vec<Responder<TaskData>>,
}

pub struct Worker {
    index: usize,
    shared_data: Arc<SharedData>,
    share_strategy: ShareStrategy,
    wait_strategy: ReceiverWaitStrategy,
    rng: RefCell<rand::XorShiftRng>,
    receiver_timeout: Duration,
    tasks: RefCell<VecDeque<Box<Task>>>,
    requester: Requester<TaskData>,
    responders: Vec<Responder<TaskData>>,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        Worker {
            index: config.index,
            shared_data: config.shared_data,
            share_strategy: config.share_strategy,
            wait_strategy: config.wait_strategy,
            rng: RefCell::new(rand::XorShiftRng::from_seed(rng::rand_seed())),
            receiver_timeout: config.receiver_timeout,
            tasks: RefCell::new(VecDeque::with_capacity(config.task_capacity)),
            requester: config.requester,
            responders: config.responders,
        }
    }

    pub fn run(&self) {
        while !self.shared_data.should_exit() {
            self.run_once();
        }

        // Run any remaining tasks if instructed to do so.
        if self.shared_data.should_run_tasks() {
            let mut is_empty = self.tasks.borrow().is_empty();

            while !is_empty {
                self.tasks.borrow_mut().pop_front().unwrap().call_box();
                is_empty = self.tasks.borrow().is_empty();
            }
        }

        // Wait on all other workers to stop running before dropping everything.
        self.shared_data.wait_on_exit();
    }

    pub fn run_once(&self) {
        if self.tasks.borrow().is_empty() {
            self.acquire_tasks();
        }
        else {
            self.tasks.borrow_mut().pop_front().unwrap().call_box();
            self.process_requests();
        }
    }

    #[inline]
    pub fn add_tasks(&self, task_data: TaskData) {
        match task_data {
            TaskData::ManyTasks(mut tasks) => {
                self.tasks.borrow_mut().append(&mut tasks);
            },
            TaskData::OneTask(task) => {
                self.tasks.borrow_mut().push_back(task);
            },
            TaskData::NoTasks => {},
        }
    }

    fn acquire_tasks(&self) {
        // Send request.
        let mut contract = self.requester.try_request().unwrap();

        // Wait for any other worker to respond or for a timeout.
        let start_time = Instant::now();

        loop {
            // See if we have received any tasks.
            if let Ok(task_data) = contract.try_receive() {
                self.add_tasks(task_data);
                break;
            }

            // While we are waiting, we should inform prospective
            // workers that we do not have any work, so they can
            // request work from another worker.
            self.process_requests();
            
            match self.wait_strategy {
                ReceiverWaitStrategy::Sleep(duration) => {
                    thread::sleep(duration);
                },
                ReceiverWaitStrategy::Yield => {
                    thread::yield_now();
                },
            }

            // If we have timed out, try to cancel the request.
            if Instant::now().duration_since(start_time) >= self.receiver_timeout {
                if contract.try_cancel() {
                    break;
                }
            }
        }
    }

    fn process_requests(&self) {
        // Start searching for requests on a random channel.
        // This *should* prevent channels that occur earlier in
        // the `responses` queue from getting preferential treatment.
        let start = self.rand_index();

        for idx in 0..self.responders.len() {
            // This makes sure the search wraps around to earlier channels.
            let index = (start + idx) % self.responders.len();
            let responder = &self.responders[index];
            
            // Share tasks with any thread that requests them.
            self.share(responder);
        }
    }

    fn share(&self, responder: &Responder<TaskData>) {
        match responder.try_respond() {
            Ok(contract) => {
                match self.share_strategy {
                    ShareStrategy::One => {
                        match self.tasks.borrow_mut().pop_front() {
                            Some(task) => {
                                contract.send(TaskData::OneTask(task));
                            },
                            None => {
                                contract.send(TaskData::NoTasks);
                            }
                        }
                    },
                    ShareStrategy::Half => {
                        let hlen = self.tasks.borrow().len() / 2;

                        if hlen > 0 {
                            contract.send(TaskData::ManyTasks(self.tasks
                                                              .borrow_mut()
                                                              .split_off(hlen)));
                        }
                        else {
                            contract.send(TaskData::NoTasks);
                        }
                    },
                }
            },
            Err(TryRespondError::NoRequest) => {},
            // There should be no clones of this `Responder`.
            Err(TryRespondError::Locked) => unreachable!(),
        }
    }

    fn rand_index(&self) -> usize {
        self.rng.borrow_mut().gen::<usize>()
            % self.responders.len()
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
    // fn add_task(&self, task: Task) {
    fn add_task(&self, task: Box<Task>) {
        self.tasks.borrow_mut().push_back(task);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Barrier};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use reqchan;
    
    use super::super::super::{ShareStrategy, ReceiverWaitStrategy, TaskData};
    use super::*;

    fn helper(share: ShareStrategy,
              receiver_timeout: Duration)
              -> (Worker, Worker) {
        let (rqst0, resps0) = reqchan::channel::<TaskData>();
        let (rqst1, resps1) = reqchan::channel::<TaskData>();

        let shared_data = Arc::new(SharedData::new(2));
        
        let worker1 = Worker::new(Config {
            index: 0,
            shared_data: shared_data.clone(),
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            receiver_timeout: receiver_timeout,
            requester: rqst0,
            responders: vec![resps1],
        });
        
        let worker2 = Worker::new(Config {
            index: 1,
            shared_data: shared_data.clone(),
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            receiver_timeout: receiver_timeout,
            requester: rqst1,
            responders: vec![resps0],
        });

        (worker1, worker2)
    }

    #[test]
    fn test_make_worker() {
        #[allow(unused_variables)]
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));
    }

    #[test]
    fn test_worker_add_task() {
        let (worker1, _) = helper(ShareStrategy::One,
                                  Duration::new(1, 0));

        worker1.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));
        assert_eq!(worker1.tasks.borrow_mut().len(), 1);
    }

    #[test]
    fn test_worker_add_tasks() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));

        worker1.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));
        assert_eq!(worker1.tasks.borrow_mut().len(), 1);

        let mut vd: VecDeque<Box<Task>> = VecDeque::new();
        vd.push_back(Box::new(|| { println!("Hello World!"); }));
        vd.push_back(Box::new(|| { println!("Hello Again!"); }));
        worker2.add_tasks(TaskData::ManyTasks(vd));
        assert_eq!(worker2.tasks.borrow_mut().len(), 2);
    }

    #[test]
    fn test_worker_share_one() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        let mut reqcon = worker2.requester.try_request().unwrap();

        worker1.share(&worker1.responders[0]);
        
        assert_eq!(worker1.tasks.borrow().len(), 0);

        if let Ok(TaskData::OneTask(task)) = reqcon.try_receive() {
            task.call_box();
        }
        else { assert!(false); }

        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_worker_share_half() {
        let (worker1, worker2) = helper(ShareStrategy::Half,
                                        Duration::new(1, 0));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();
        let var3 = var.clone();
        let var4 = var.clone();

        let mut vd: VecDeque<Box<Task>> = VecDeque::new();
        vd.push_back(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }));
        vd.push_back(Box::new(move || {
            var2.fetch_add(2, Ordering::SeqCst);
        }));
        vd.push_back(Box::new(move || {
            var3.fetch_add(3, Ordering::SeqCst);
        }));
        vd.push_back(Box::new(move || {
            var4.fetch_add(4, Ordering::SeqCst);
        }));
        worker1.add_tasks(TaskData::ManyTasks(vd));

        let mut reqcon = worker2.requester.try_request().unwrap();

        worker1.share(&worker1.responders[0]);

        assert_eq!(worker1.tasks.borrow().len(), 2);

        if let Ok(TaskData::ManyTasks(mut tasks)) = reqcon.try_receive() {
            tasks.pop_front().unwrap().call_box();

            assert_eq!(var.load(Ordering::SeqCst), 3);

            tasks.pop_front().unwrap().call_box();

            assert_eq!(var.load(Ordering::SeqCst), 7);
        }
        else { assert!(false); }
    }

    #[test]
    fn test_worker_acquire_tasks() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));

        let stop1 = Arc::new(AtomicBool::new(false));
        let stop2 = stop1.clone();

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        let mut tasks: VecDeque<Box<Task>> = VecDeque::new();
        tasks.push_back(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }));

        let handle = thread::spawn(move || {
            worker2.acquire_tasks();

            assert_eq!(worker2.tasks.borrow().len(), 1);

            worker2.tasks.borrow_mut().pop_front().unwrap().call_box();

            stop2.store(true, Ordering::SeqCst);
        });

        while !stop1.load(Ordering::SeqCst) {
            if let Ok(respcon) = worker1.responders[0].try_respond() {
                respcon.send(TaskData::OneTask(tasks.pop_front().unwrap()));
            }
        }

        handle.join().unwrap();

        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_worker_acquire_tasks_timeout() {
        #[allow(unused_variables)]
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));

        worker1.acquire_tasks();
    }

    #[test]
    fn test_worker_process_requests() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));

        let stop1 = Arc::new(AtomicBool::new(false));
        let stop2 = stop1.clone();

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        let handle = thread::spawn(move || {
            let mut reqcon = worker2.requester.try_request().unwrap();

            while !stop2.load(Ordering::SeqCst) {
                if let Ok(TaskData::OneTask(task)) = reqcon.try_receive() {
                    task.call_box();
                    stop2.store(true, Ordering::SeqCst);
                }
            }
        });

        while !stop1.load(Ordering::SeqCst) {
            worker1.process_requests();
        }

        handle.join().unwrap();

        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_worker_run_once() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(5, 0));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();
        let var3 = var.clone();
        let var4 = var.clone();

        let mut vd: VecDeque<Box<Task>> = VecDeque::new();
        vd.push_back(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }));
        vd.push_back(Box::new(move || {
            var2.fetch_add(2, Ordering::SeqCst);
        }));
        vd.push_back(Box::new(move || {
            var3.fetch_add(3, Ordering::SeqCst);
        }));
        worker1.add_tasks(TaskData::ManyTasks(vd));

        // Execute 1st task.
        worker1.run_once();
        
        assert_eq!(worker1.tasks.borrow().len(), 2);
        assert_eq!(var.load(Ordering::SeqCst), 1);

        let handle = thread::spawn(move || {
            // Get 3rd task.
            worker2.run_once();

            assert_eq!(worker2.tasks.borrow().len(), 1);
            assert_eq!(var4.load(Ordering::SeqCst), 3);

            // Execute 3rd task.
            worker2.run_once();
        });

        // Execute 2nd task and share 3rd task.
        thread::sleep(Duration::new(2, 0));
        worker1.run_once();
        
        assert_eq!(worker1.tasks.borrow().len(), 0);

        // Wait until thread2 exits.
        handle.join().unwrap();

        assert_eq!(var.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn test_worker_run_signal_exit() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0));


        let handle = thread::spawn(move || {
            worker2.run();
        });
        
        thread::sleep(Duration::new(0, 100));
        worker1.signal_exit();
        worker1.shared_data.wait_on_exit();

        handle.join().unwrap();
    }
}

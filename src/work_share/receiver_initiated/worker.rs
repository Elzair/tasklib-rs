use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rand;
use rand::{Rng, SeedableRng};

use super::super::super::{ShareStrategy, ReceiverWaitStrategy};
use super::super::super::Worker as WorkerTrait;
use super::super::channel::boolean as bc;
use super::super::super::task::Task;
use super::super::task::Data as TaskData;
use super::super::util;
use super::channel::{Channel, Data};
use super::shared::Data as SharedData;

pub struct Config {
    pub index: usize,
    pub shared_data: Arc<SharedData>,
    pub task_capacity: usize,
    pub share_strategy: ShareStrategy,
    pub wait_strategy: ReceiverWaitStrategy,
    pub receiver_timeout: Duration,
    pub channel_timeout: Duration,
    pub channel_data: Data,
}

pub struct Worker {
    index: usize,
    shared_data: Arc<SharedData>,
    share_strategy: ShareStrategy,
    wait_strategy: ReceiverWaitStrategy,
    rng: RefCell<rand::XorShiftRng>,
    receiver_timeout: Duration,
    channel_timeout: Duration,
    tasks: RefCell<VecDeque<Task>>,
    channel_data: Data,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        Worker {
            index: config.index,
            shared_data: config.shared_data,
            share_strategy: config.share_strategy,
            wait_strategy: config.wait_strategy,
            rng: RefCell::new(rand::XorShiftRng::from_seed(util::rand_seed())),
            receiver_timeout: config.receiver_timeout,
            channel_timeout: config.channel_timeout,
            tasks: RefCell::new(VecDeque::with_capacity(config.task_capacity)),
            channel_data: config.channel_data,
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
    fn add_tasks(&self, task_data: TaskData) {
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

    fn try_get_tasks(&self, channel: &Channel) -> bool {
        let start_time = Instant::now();

        loop {
            if let Ok(task_data) = channel.task_get.try_receive() {
                self.add_tasks(task_data);
                return true;
            }

            match self.wait_strategy {
                ReceiverWaitStrategy::Sleep(duration) => {
                    thread::sleep(duration);
                },
                ReceiverWaitStrategy::Yield => {
                    thread::yield_now();
                },
            }

            if Instant::now().duration_since(start_time) >= self.channel_timeout {
                return false;
            }
        }
    }

    fn acquire_tasks(&self) {
        let start_time = Instant::now();
        let mut done = false;

        while !done {
            let rand_idx = self.rand_index();
            let channel = &self.channel_data.channels[rand_idx];

            // Send request.
            channel.request_send.send().unwrap();
            
            // Wait for either tasks or timeout.
            match self.try_get_tasks(channel) {
                true => {
                    done = true;
                },
                false => {
                    // Try to unsend request.
                    match channel.request_send.try_unsend() {
                        // If we are too late, just block until we get all the tasks.
                        Err(bc::TryUnsendError::TooLate) => {
                            self.add_tasks(channel.task_get.receive());
                        },
                        _ => {},
                    }
                },
            }

            if Instant::now().duration_since(start_time) >= self.receiver_timeout {
                done = true;
            }
        }
    }

    fn process_requests(&self) {
        // Start searching for requests on a random channel.
        // This *should* prevent channels that occur earlier in
        // the `channel_data.channels` queue from getting
        // preferential treatment.
        let start = self.rand_index();
        let len = self.channel_data.channels.len();

        for idx in 0..len {
            // This makes sure the search wraps around to earlier channels.
            let index = (start + idx) % len;
            let channel = &self.channel_data.channels[index];

            // Do not trying sharing tasks if we do not have any.
            if self.tasks.borrow().is_empty() {
                break;
            }
            
            // Share tasks with any thread that requests them.
            if channel.request_get.receive() == true {
                self.share(channel);
            }
        }
    }

    fn share(&self, channel: &Channel) {
        match self.share_strategy {
            ShareStrategy::One => {
                channel.task_send.send(
                    TaskData::OneTask(self.tasks.borrow_mut().pop_front()
                                      .unwrap())
                ).unwrap();
            },
            ShareStrategy::Half => {
                let half_len = self.tasks.borrow().len() / 2;

                if half_len > 0 {
                    channel.task_send.send(
                        TaskData::ManyTasks(self.tasks.borrow_mut()
                                            .split_off(half_len))
                    ).unwrap();
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        self.rng.borrow_mut().gen::<usize>()
            % self.channel_data.channels.len()
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
        self.tasks.borrow_mut().push_back(task);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;
    
    // use super::super::super::super::{ShareStrategy, ReceiverWaitStrategy};
    // use super::super::super::super::task::Data as TaskData;
    use super::super::super::super::task::Task;
    use super::super::channel::make_channels;
    use super::super::shared::Data as SharedData;
    use super::*;

    fn helper(share: ShareStrategy,
              receiver_timeout: Duration,
              channel_timeout: Duration)
              -> (Worker, Worker) {
        let mut channels = make_channels(2);
        let shared_data = Arc::new(SharedData::new(1));
        
        let worker1 = Worker::new(Config {
            index: 0,
            shared_data: shared_data.clone(),
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            receiver_timeout: receiver_timeout,
            channel_timeout: channel_timeout,
            channel_data: channels.remove(0),
        });
        
        let worker2 = Worker::new(Config {
            index: 1,
            shared_data: shared_data.clone(),
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            receiver_timeout: receiver_timeout,
            channel_timeout: channel_timeout,
            channel_data: channels.remove(0),
        });

        (worker1, worker2)
    }

    #[test]
    fn test_make_worker() {
        #[allow(unused_variables)]
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));
    }

    #[test]
    fn test_worker_add_task() {
        let (worker1, _) = helper(ShareStrategy::One,
                                  Duration::new(1, 0),
                                  Duration::new(0, 100));

        worker1.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));
        assert_eq!(worker1.tasks.borrow_mut().len(), 1);
    }

    #[test]
    fn test_worker_add_tasks() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));

        worker1.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));
        assert_eq!(worker1.tasks.borrow_mut().len(), 1);

        let mut vd = VecDeque::new();
        vd.push_back(Box::new(|| { println!("Hello World!"); }) as Task);
        vd.push_back(Box::new(|| { println!("Hello Again!"); }) as Task);
        worker2.add_tasks(TaskData::ManyTasks(vd));
        assert_eq!(worker2.tasks.borrow_mut().len(), 2);
    }

    #[test]
    fn test_worker_share_one() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        worker1.share(&worker1.channel_data.channels[0]);
        assert_eq!(worker1.tasks.borrow().len(), 0);

        worker2.add_tasks(worker2.channel_data.channels[0].task_get.receive());
        assert_eq!(worker2.tasks.borrow().len(), 1);

        worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_worker_share_half() {
        let (worker1, worker2) = helper(ShareStrategy::Half,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();
        let var3 = var.clone();
        let var4 = var.clone();

        let mut vd = VecDeque::new();
        vd.push_back(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }) as Task);
        vd.push_back(Box::new(move || {
            var2.fetch_add(2, Ordering::SeqCst);
        }) as Task);
        vd.push_back(Box::new(move || {
            var3.fetch_add(3, Ordering::SeqCst);
        }) as Task);
        vd.push_back(Box::new(move || {
            var4.fetch_add(4, Ordering::SeqCst);
        }) as Task);
        worker1.add_tasks(TaskData::ManyTasks(vd));

        worker1.share(&worker1.channel_data.channels[0]);
        assert_eq!(worker1.tasks.borrow().len(), 2);

        worker2.add_tasks(worker2.channel_data.channels[0].task_get.receive());
        assert_eq!(worker2.tasks.borrow().len(), 2);

        worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
        assert_eq!(var.load(Ordering::SeqCst), 3);
        worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
        assert_eq!(var.load(Ordering::SeqCst), 7);
    }

    #[test]
    fn test_worker_try_get_tasks() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));


        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        worker1.share(&worker1.channel_data.channels[0]);

        let res = worker2.try_get_tasks(&worker2.channel_data.channels[0]);
        assert_eq!(res, true);

        worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
        assert_eq!(var.load(Ordering::SeqCst), 1);

        let res = worker2.try_get_tasks(&worker2.channel_data.channels[0]);
        assert_eq!(res, false);
    }

    #[test]
    fn test_worker_acquire_tasks() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        let handle = thread::spawn(move || {
            worker2.acquire_tasks();

            assert_eq!(worker2.tasks.borrow().len(), 1);

            worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
            assert_eq!(var.load(Ordering::SeqCst), 1);
        });

        let mut done = false;

        while done == false {
            let res = worker1.channel_data.channels[0].request_get.receive();

            if res == true {
                worker1.share(&worker1.channel_data.channels[0]);
                done = true;
            }
        }

        handle.join().unwrap();
    }

    #[test]
    fn test_worker_process_requests() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(1, 0),
                                        Duration::new(0, 100));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();

        worker1.add_tasks(TaskData::OneTask(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        })));

        worker2.channel_data.channels[0].request_send.send().unwrap();

        worker1.process_requests();

        worker2.add_tasks(worker2.channel_data.channels[0].task_get.receive());

        worker2.tasks.borrow_mut().pop_front().unwrap().call_box();
        assert_eq!(var.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_worker_run_once() {
        let (worker1, worker2) = helper(ShareStrategy::One,
                                        Duration::new(3, 0),
                                        Duration::new(1, 0));

        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();
        let var3 = var.clone();
        let var4 = var.clone();

        let mut vd = VecDeque::new();
        vd.push_back(Box::new(move || {
            var1.fetch_add(1, Ordering::SeqCst);
        }) as Task);
        vd.push_back(Box::new(move || {
            var2.fetch_add(2, Ordering::SeqCst);
        }) as Task);
        vd.push_back(Box::new(move || {
            var3.fetch_add(3, Ordering::SeqCst);
        }) as Task);
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
}

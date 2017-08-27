use std::cell::RefCell;
use std::collections::VecDeque;
use std::thread;
use std::time::{Duration, Instant};

use rand;
use rand::{Rng, SeedableRng};

use super::{ShareStrategy, ReceiverWaitStrategy};
use super::channel::boolchan as bc;
use super::channel::{DataRI, DataSI};
use super::task::Task;
use super::task::Data as TaskData;

pub struct ConfigRI {
    pub index: usize,
    pub task_capacity: usize,
    pub share_strategy: ShareStrategy,
    pub wait_strategy: ReceiverWaitStrategy,
    pub timeout: Option<Duration>,
    pub channel_data: DataRI,
}

pub struct WorkerRI {
    pub index: usize,
    share_strategy: ShareStrategy,
    wait_strategy: ReceiverWaitStrategy,
    rng: RefCell<rand::XorShiftRng>,
    timeout: Option<Duration>,
    tasks: RefCell<VecDeque<Task>>,
    channel_data: DataRI,
}

impl WorkerRI {
    pub fn new(config: ConfigRI) -> WorkerRI {
        let ConfigRI { index,
                       task_capacity,
                       share_strategy,
                       wait_strategy,
                       timeout,
                       channel_data,
        } = config;

        // Generate the seed of the worker's RNG.
        let mut tmp_rng = rand::thread_rng();
        let mut seed: [u32; 4] = [0, 0, 0, 0];
        let mut good_seed = false;

        while good_seed == false {
            seed[0] = tmp_rng.gen::<u32>();
            seed[1] = tmp_rng.gen::<u32>();
            seed[2] = tmp_rng.gen::<u32>();
            seed[3] = tmp_rng.gen::<u32>();

            // `rand::XorShiftRng` will panic if the seed is all zeroes.
            // Ensure that does not happen.
            if !(seed[0] == 0 && seed[1] == 0 && seed[2] == 0 && seed[3] == 0) {
                good_seed = true;
            }
        }

        WorkerRI {
            index: index,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            rng: RefCell::new(rand::XorShiftRng::from_seed(seed)),
            timeout: timeout,
            tasks: RefCell::new(VecDeque::with_capacity(task_capacity)),
            channel_data: channel_data,
        }
    }

    pub fn run(&self) {
        loop {
            self.run_once();
        }
    }

    #[inline]
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
    pub fn add_tasks(&self, task_data: TaskData) -> bool {
        match task_data {
            TaskData::ManyTasks(mut tasks) => {
                self.tasks.borrow_mut().append(&mut tasks);
                true
            },
            TaskData::OneTask(task) => {
                self.tasks.borrow_mut().push_back(task);
                true
            },
            TaskData::NoTasks => { false },
        }
    }

    fn try_get_tasks(&self, index: usize) -> bool {
        let start_time = Instant::now();

        loop {
            if let Ok(task_data) = self.channel_data.channels[index]
                .task_get.try_receive() {
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
            };

            if let Some(timeout) = self.timeout {
                if Instant::now().duration_since(start_time) >= timeout {
                    return false;
                }
            }
        }
    }

    fn acquire_tasks(&self) {
        let mut got_tasks = false;

        while !got_tasks {
            let rand_idx = self.rand_index();

            // Send request.
            self.channel_data.channels[rand_idx].request_send.send().unwrap();
            
            // Wait for either tasks or timeout.
            match self.try_get_tasks(rand_idx) {
                true => {
                    got_tasks = true;
                },
                false => {
                    // Try to unsend request.
                    match self.channel_data.channels[rand_idx]
                        .request_send.try_unsend() {
                            // If we are too late, just block until we get all the tasks.
                            Err(bc::TryUnsendError::TooLate) => {
                                self.add_tasks(self.channel_data.channels[rand_idx]
                                               .task_get.receive());
                            },
                            _ => {},
                        }
                },
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

            // Do not trying sharing tasks if we do not have any.
            if self.tasks.borrow().is_empty() {
                break;
            }
            
            // Share tasks with any thread that requests them.
            if self.channel_data.channels[index]
                .request_get.receive() == true {
                    self.share(index);
            }
        }
    }

    fn share(&self, idx: usize) {
        match self.share_strategy {
            ShareStrategy::One => {
                self.channel_data.channels[idx].task_send.send(
                    TaskData::OneTask(self.tasks.borrow_mut().pop_front()
                                      .unwrap())
                ).unwrap();
            },
            ShareStrategy::Half => {
                let half_len = self.tasks.borrow().len() / 2;

                if half_len > 0 {
                    self.channel_data.channels[idx].task_send.send(
                        TaskData::ManyTasks(self.tasks.borrow_mut()
                                            .split_off(half_len))
                    ).unwrap();
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        (self.rng.borrow_mut().next_u32() as usize)
            % self.channel_data.channels.len()
    }
}

pub struct ConfigSI {
    pub index: usize,
    pub task_capacity: usize,
    pub share_strategy: ShareStrategy,
    pub wait_strategy: ReceiverWaitStrategy,
    pub channel_data: DataSI,
}

pub struct WorkerSI {
    pub index: usize,
    share_strategy: ShareStrategy,
    wait_strategy: ReceiverWaitStrategy,
    rng: RefCell<rand::XorShiftRng>,
    tasks: RefCell<VecDeque<Task>>,
    channel_data: DataSI,
}

impl WorkerSI {
    pub fn new(config: ConfigSI) -> WorkerSI {
        let ConfigSI { index,
                       task_capacity,
                       share_strategy,
                       wait_strategy,
                       channel_data,
        } = config;

        // Generate the seed of the worker's RNG.
        let mut tmp_rng = rand::thread_rng();
        let mut seed: [u32; 4] = [0, 0, 0, 0];
        let mut good_seed = false;

        while good_seed == false {
            seed[0] = tmp_rng.gen::<u32>();
            seed[1] = tmp_rng.gen::<u32>();
            seed[2] = tmp_rng.gen::<u32>();
            seed[3] = tmp_rng.gen::<u32>();

            // `rand::XorShiftRng` will panic if the seed is all zeroes.
            // Ensure that does not happen.
            if !(seed[0] == 0 && seed[1] == 0 && seed[2] == 0 && seed[3] == 0) {
                good_seed = true;
            }
        }

        WorkerSI {
            index: index,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            rng: RefCell::new(rand::XorShiftRng::from_seed(seed)),
            tasks: RefCell::new(VecDeque::with_capacity(task_capacity)),
            channel_data: channel_data,
        }
    }

    pub fn run(&self) {
        loop {
            self.run_once();
        }
    }

    #[inline]
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
    pub fn add_tasks(&self, task_data: TaskData) -> bool {
        match task_data {
            TaskData::ManyTasks(mut tasks) => {
                self.tasks.borrow_mut().append(&mut tasks);
                true
            },
            TaskData::OneTask(task) => {
                self.tasks.borrow_mut().push_back(task);
                true
            },
            TaskData::NoTasks => { false },
        }
    }

    fn acquire_tasks(&self) {
        // Send request.
        self.channel_data.request_send.send().unwrap();

        // Wait for any other worker to respond.
        let mut got_tasks = false;

        while got_tasks == false {
            for channel in self.channel_data.channels.iter() {
                if let Ok(task_data) = channel.task_get.try_receive() {
                    self.add_tasks(task_data);
                    got_tasks = true;
                }
            }

            if got_tasks == false {
                match self.wait_strategy {
                    ReceiverWaitStrategy::Sleep(duration) => {
                        thread::sleep(duration);
                    },
                    ReceiverWaitStrategy::Yield => {
                        thread::yield_now();
                    },
                };
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

            // Do not trying sharing tasks if we do not have any.
            if self.tasks.borrow().is_empty() {
                break;
            }
            
            // Share tasks with any thread that requests them.
            if self.channel_data.channels[index]
                .request_get.receive() == true {
                    self.share(index);
            }
        }
    }

    fn share(&self, index: usize) {
        match self.share_strategy {
            ShareStrategy::One => {
                self.channel_data.channels[index].task_send.send(
                    TaskData::OneTask(self.tasks.borrow_mut().pop_front()
                                      .unwrap())
                ).unwrap();
            },
            ShareStrategy::Half => {
                let half_len = self.tasks.borrow().len() / 2;

                if half_len > 0 {
                    self.channel_data.channels[index].task_send.send(
                        TaskData::ManyTasks(self.tasks.borrow_mut()
                                            .split_off(half_len))
                    ).unwrap();
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        (self.rng.borrow_mut().next_u32() as usize)
            % self.channel_data.channels.len()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    
    use super::{ConfigRI, ConfigSI, WorkerRI, WorkerSI};
    use super::super::{ShareStrategy, ReceiverWaitStrategy};
    use super::super::channel::{make_receiver_initiated_channels, make_sender_initiated_channels};
    use super::super::task::Data as TaskData;

    fn helper_ri(share: ShareStrategy,
                 timeout: Option<Duration>)
                 -> (WorkerRI, WorkerRI) {
        let mut channels = make_receiver_initiated_channels(2);
        
        let worker1 = WorkerRI::new(ConfigRI {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            timeout: timeout,
            channel_data: channels.remove(0),
        });
        
        let worker2 = WorkerRI::new(ConfigRI {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            timeout: timeout,
            channel_data: channels.remove(0),
        });

        (worker1, worker2)
    }

    fn helper_si(share: ShareStrategy)
                 -> (WorkerSI, WorkerSI) {
        let mut channels = make_sender_initiated_channels(2);
        
        let worker1 = WorkerSI::new(ConfigSI {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            channel_data: channels.remove(0),
        });
        
        let worker2 = WorkerSI::new(ConfigSI {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            wait_strategy: ReceiverWaitStrategy::Yield,
            channel_data: channels.remove(0),
        });

        (worker1, worker2)
    }

    #[test]
    fn test_make_worker_ri() {
        #[allow(unused_variables)]
        let (worker1, worker2) = helper_ri(ShareStrategy::One, None);
    }

    #[test]
    fn test_make_worker_si() {
        #[allow(unused_variables)]
        let (worker1, worker2) = helper_si(ShareStrategy::One);
    }

    #[test]
    fn test_worker_ri_addtask() {
        let (worker, _) = helper_ri(ShareStrategy::One, None);

        worker.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));

        assert!(worker.tasks.borrow_mut().len() == 1);
    }

    #[test]
    fn test_worker_si_addtask() {
        let (worker, _) = helper_si(ShareStrategy::One);

        worker.add_tasks(TaskData::OneTask(Box::new(|| { println!("Hello World!");})));

        assert!(worker.tasks.borrow_mut().len() == 1);
    }

}

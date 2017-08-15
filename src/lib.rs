use std::sync::{Arc,Mutex,mpsc};
use std::thread;

extern crate itertools;
extern crate rand;

pub mod task;
mod worker;
pub mod pool;

pub use task::Task;

// pub struct ThreadPool {
//     workers: Vec<Worker>,
//     sender:  mpsc::Sender<Message>,
// }

// impl ThreadPool {
//     /// Create a new ThreadPool.
//     ///
//     /// The size is the number of threads in the pool.
//     ///
//     /// # Panics
//     ///
//     /// The `new` function will panic if the size is zero.
//     pub fn new(size: usize) -> ThreadPool {
//         assert!(size > 0);

//         let (sender, receiver) = mpsc::channel();
//         let receiver = Arc::new(Mutex::new(receiver));

//         let mut workers = Vec::with_capacity(size);

//         for id in 0..size {
//             // Create some threads and store them in the vector
//             workers.push(Worker::new(id, receiver.clone()));
//         }
        
//         ThreadPool {
//             workers,
//             sender
//         }
//     }

//     pub fn execute<F>(&self, f: F)
//         where
//         F: FnOnce() + Send + 'static
//     {
//         let job = Box::new(f);

//         self.sender.send(Message::NewJob(job)).unwrap();
//     }
// }

// impl Drop for ThreadPool {
//     fn drop(&mut self) {
//         println!("Sending terminate message to all workers.");

//         for _ in &mut self.workers {
//             self.sender.send(Message::Terminate).unwrap();
//         }
        
//         for worker in &mut self.workers {
//             println!("Shutting down worker {}", worker.id);

//             if let Some(thread) = worker.thread.take() {
//                 thread.join().unwrap();
//             }
//         }
//     }
// }

// struct Worker {
//     id:     usize,
//     thread: Option<thread::JoinHandle<()>>,
// }

// impl Worker {
//     fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
//         let thread = thread::spawn(move || {
//             loop {
//                 let message = receiver.lock().unwrap().recv().unwrap();

//                 match message {
//                     Message::NewJob(job) => {
//                         println!("Worker {} got a job; executing.", id);

//                         job.call_box();
//                     },
//                     Message::Terminate => {
//                         println!("Worker {} is terminating.", id);
//                         break;
//                     },
//                 }
//             }
//         });

//         Worker {
//             id:     id,
//             thread: Some(thread),
//         }
//     }
// }

// enum Message {
//     NewJob(Job),
//     Terminate,
// }

// type Job = Box<FnBox + Send + 'static>;

// /// To call an `FnOnce` closure stored in a `Box`, the closure needs to move
// /// itself out of the `Box` since it takes ownership of itself when called.
// /// Until and unl
// /// `Box`
// trait FnBox {
//     fn call_box(self: Box<Self>);
// }

// impl<F: FnOnce()> FnBox for F {
//     fn call_box(self: Box<F>) {
//         (*self)()
//     }
// }

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct MyThreadPool {
    sender: Option<mpsc::Sender<Job>>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl MyThreadPool {
    fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            let receiver_clone = Arc::clone(&receiver);

            let handle = thread::spawn(move || {
                let thread_id = thread::current().id();
                println!("- Worker thread started: {:?}", thread_id);

                loop {
                    let message = {
                        let lock = receiver_clone.lock().unwrap();
                        lock.recv()
                    };

                    match message {
                        Ok(job) => {
                            job();
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            });
            workers.push(handle);
        }

        MyThreadPool {
            sender: Some(sender),
            workers,
        }
    }

    fn post<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(sender) = &self.sender {
            sender.send(Box::new(f)).expect("ThreadPool is closed");
        }
    }

    fn send<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let done = Arc::new(AtomicBool::new(false));
        let done_clone = done.clone();

        self.post(move || {
            f();
            done_clone.store(true, Ordering::Release);
        });

        while !done.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
    }
}

impl Drop for MyThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.take() {
                thread.join().unwrap();
            }
        }
    }
}

fn main() {
    println!("Main Thread ID: {:?}", thread::current().id());

    let pool = MyThreadPool::new(4);

    thread::sleep(Duration::from_millis(100));
    println!("\nStarting Async Post\n");
    for i in 0..10 {
        pool.post(move || {
            println!(
                "- Post Task #{} executing on {:?}",
                i,
                thread::current().id()
            );
            thread::sleep(Duration::from_millis(10));
        });
    }

    thread::sleep(Duration::from_millis(500));

    println!("\nStarting Sync Send\n");

    for i in 0..3 {
        pool.send(move || {
            println!(
                "- Send Task #{} executing on {:?}",
                i,
                thread::current().id()
            );
            thread::sleep(Duration::from_millis(200));
        });
        println!("Send #{} finished. Main thread continues.", i);
    }

    println!("\nFinished");
}

trait JoinHandleExt {
    fn take(&mut self) -> Option<thread::JoinHandle<()>>;
}

impl JoinHandleExt for thread::JoinHandle<()> {
    fn take(&mut self) -> Option<thread::JoinHandle<()>> {
        None
    }
}

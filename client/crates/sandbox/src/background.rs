//! Work kept off the frame: the loaders' jobs. Natively each loader is a
//! thread of its own, as it always was. The browser has no threads
//! (docs/web-spike.md): there the jobs run on the page's loop when their
//! results are polled, a few milliseconds a frame (`new_frame`), and a job
//! that met a game file still downloading (`web_files`) runs again once the
//! files are in.

#[cfg(not(target_arch = "wasm32"))]
pub use std::sync::mpsc::{Receiver as Results, Sender as Requests};

/// Starts `handler` on the jobs sent to the returned requests; its answers
/// come out of the returned results.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<J: Send + 'static, R: Send + 'static>(
    name: &str,
    mut handler: impl FnMut(J) -> R + Send + 'static,
) -> std::io::Result<(Requests<J>, Results<R>)> {
    let (request_tx, request_rx) = std::sync::mpsc::channel::<J>();
    let (result_tx, result_rx) = std::sync::mpsc::channel::<R>();
    std::thread::Builder::new().name(name.to_owned()).spawn(move || {
        while let Ok(job) = request_rx.recv() {
            if result_tx.send(handler(job)).is_err() {
                break;
            }
        }
    })?;
    Ok((request_tx, result_rx))
}

#[cfg(target_arch = "wasm32")]
pub use web::{Requests, Results, new_frame, spawn};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        rc::Rc,
        sync::mpsc::{SendError, TryRecvError},
    };

    /// Milliseconds of each frame the jobs may take.
    const BUDGET_MS: f64 = 8.0;

    thread_local! {
        static DEADLINE: Cell<f64> = const { Cell::new(0.0) };
    }

    fn now() -> f64 {
        js_sys::Date::now()
    }

    /// A frame begins: the jobs get their few milliseconds again.
    pub fn new_frame() {
        DEADLINE.with(|deadline| deadline.set(now() + BUDGET_MS));
    }

    fn in_budget() -> bool {
        DEADLINE.with(|deadline| now() < deadline.get())
    }

    pub struct Requests<J> {
        jobs: Rc<RefCell<VecDeque<J>>>,
    }

    impl<J> Requests<J> {
        pub fn send(&self, job: J) -> Result<(), SendError<J>> {
            self.jobs.borrow_mut().push_back(job);
            Ok(())
        }
    }

    pub struct Results<R> {
        step: RefCell<Box<dyn FnMut() -> Option<R>>>,
    }

    impl<R> Results<R> {
        /// The next answer, worked out now if the frame has time left.
        pub fn try_recv(&self) -> Result<R, TryRecvError> {
            if !in_budget() {
                return Err(TryRecvError::Empty);
            }
            (self.step.borrow_mut())().ok_or(TryRecvError::Empty)
        }

        pub fn try_iter(&self) -> impl Iterator<Item = R> + '_ {
            std::iter::from_fn(|| self.try_recv().ok())
        }
    }

    pub fn spawn<J: Clone + 'static, R: 'static>(
        _name: &str,
        mut handler: impl FnMut(J) -> R + 'static,
    ) -> std::io::Result<(Requests<J>, Results<R>)> {
        let jobs = Rc::new(RefCell::new(VecDeque::<J>::new()));
        let queue = jobs.clone();
        // Jobs that met a file on its way, run again when the files are in.
        let mut waiting: Vec<J> = Vec::new();
        let step = move || {
            if !waiting.is_empty() && !crate::web_files::busy() {
                let mut jobs = queue.borrow_mut();
                for job in waiting.drain(..).rev() {
                    jobs.push_front(job);
                }
            }
            loop {
                let job = queue.borrow_mut().pop_front()?;
                let misses = crate::web_files::misses();
                let answer = handler(job.clone());
                if crate::web_files::misses() == misses {
                    return Some(answer);
                }
                waiting.push(job);
                if !in_budget() {
                    return None;
                }
            }
        };
        Ok((Requests { jobs }, Results { step: RefCell::new(Box::new(step)) }))
    }
}

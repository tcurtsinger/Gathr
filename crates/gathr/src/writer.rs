//! The one writer: a thread that owns the only write connection and takes work in order (0003).

use std::sync::mpsc;

use rusqlite::Connection;

use crate::crash_log::CrashLog;
use crate::event::Event;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

#[derive(Clone)]
pub struct Writer {
    jobs: mpsc::Sender<Job>,
    crash_log: CrashLog,
}

impl Writer {
    /// Starts the writer thread. It runs until every `Writer` handle is dropped. `after` runs after
    /// each job, once its writes are committed.
    pub fn start(
        mut conn: Connection,
        crash_log: CrashLog,
        mut after: impl FnMut(&mut Connection) + Send + 'static,
    ) -> Writer {
        let (jobs, queue) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("gathr-writer".into())
            .spawn(move || {
                for job in queue {
                    job(&mut conn);
                    after(&mut conn);
                }
            })
            .expect("the writer thread didn't start");
        Writer { jobs, crash_log }
    }

    /// Queues an event without waiting. If it can't be written, it goes to the crash log.
    pub fn record(&self, event: Event) {
        let crash_log = self.crash_log.clone();
        let job: Job = Box::new(move |conn| {
            if let Err(e) = event.insert(conn) {
                crash_log.append(&format!("event {} not recorded: {e}", event.name));
            }
        });
        if let Err(mpsc::SendError(job)) = self.jobs.send(job) {
            drop(job);
            self.crash_log
                .append("event not recorded: the writer thread has stopped");
        }
    }

    /// Writes an event and waits until it's committed, so whatever reads next can find it.
    pub async fn record_now(&self, event: Event) {
        let crash_log = self.crash_log.clone();
        self.call(move |conn| {
            if let Err(e) = event.insert(conn) {
                crash_log.append(&format!("event {} not recorded: {e}", event.name));
            }
        })
        .await;
    }

    /// Runs `work` on the writer thread and returns its result; `None` if the thread has stopped.
    pub async fn call<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> T + Send + 'static,
    ) -> Option<T> {
        let (done, result) = tokio::sync::oneshot::channel();
        let job: Job = Box::new(move |conn| {
            let _ = done.send(work(conn));
        });
        self.jobs.send(job).ok()?;
        result.await.ok()
    }
}

//! The Chrome DevTools Protocol over the browser's debugging pipe: JSON messages each ended by a NUL
//! byte, a call's answer carrying the call's id, and events by their method.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

/// How long a call waits for its answer.
const ANSWER: Duration = Duration::from_secs(30);

type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// An event the browser sent: its method, its params, and the page's session it came from.
pub struct Heard {
    pub method: String,
    pub params: Value,
    pub session: Option<String>,
}

pub struct Cdp {
    out: mpsc::UnboundedSender<Vec<u8>>,
    waiting: Waiting,
    next: AtomicU64,
}

impl Cdp {
    /// Talks over the pipe's two ends; the events come out of the receiver until the pipe closes.
    pub fn start(
        write: impl AsyncWrite + Unpin + Send + 'static,
        read: impl AsyncRead + Unpin + Send + 'static,
    ) -> (Arc<Cdp>, mpsc::UnboundedReceiver<Heard>) {
        let (out, mut sending) = mpsc::unbounded_channel::<Vec<u8>>();
        let waiting: Waiting = Arc::default();
        let (events, heard) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut write = write;
            while let Some(mut message) = sending.recv().await {
                message.push(0);
                if write.write_all(&message).await.is_err() || write.flush().await.is_err() {
                    return;
                }
            }
        });
        let answers = waiting.clone();
        tokio::spawn(async move {
            let mut read = BufReader::new(read);
            let mut message = Vec::new();
            loop {
                message.clear();
                match read.read_until(0, &mut message).await {
                    Ok(n) if n > 0 && message.last() == Some(&0) => {}
                    _ => break,
                }
                message.pop();
                let Ok(value) = serde_json::from_slice::<Value>(&message) else {
                    continue;
                };
                if let Some(id) = value["id"].as_u64() {
                    let answer = match value.get("error") {
                        Some(error) => {
                            Err(error["message"].as_str().unwrap_or("failed").to_string())
                        }
                        None => Ok(value["result"].clone()),
                    };
                    let waiter = answers.lock().map(|mut w| w.remove(&id)).ok().flatten();
                    if let Some(waiter) = waiter {
                        let _ = waiter.send(answer);
                    }
                    continue;
                }
                let Some(method) = value["method"].as_str() else {
                    continue;
                };
                let heard = Heard {
                    method: method.to_string(),
                    session: value["sessionId"].as_str().map(String::from),
                    params: value["params"].clone(),
                };
                if events.send(heard).is_err() {
                    break;
                }
            }
            // The pipe closed: every call still waiting fails at once.
            if let Ok(mut waiting) = answers.lock() {
                waiting.clear();
            }
        });
        let cdp = Cdp {
            out,
            waiting,
            next: AtomicU64::new(1),
        };
        (Arc::new(cdp), heard)
    }

    /// Calls `method`, on the page's `session` when given: it is queued now, in the order of the calls,
    /// and the future waits for its answer.
    pub fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> impl Future<Output = Result<Value, String>> + Send + use<> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (answer, answered) = oneshot::channel();
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.insert(id, answer);
        }
        let sent = self.out.send(message(id, method, params, session)).is_ok();
        let (waiting, method) = (self.waiting.clone(), method.to_string());
        async move {
            let answer = match sent {
                true => tokio::time::timeout(ANSWER, answered).await,
                false => Ok(Ok(Err("the browser closed".into()))),
            };
            if let Ok(mut waiting) = waiting.lock() {
                waiting.remove(&id);
            }
            match answer {
                Ok(Ok(answer)) => answer,
                Ok(Err(_)) => Err("the browser closed".into()),
                Err(_) => Err(format!("{method} wasn't answered")),
            }
        }
    }

    /// Calls `method` without waiting for its answer: queued now, as `call` is.
    pub fn send(&self, method: &str, params: Value, session: Option<&str>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.out.send(message(id, method, params, session));
    }
}

fn message(id: u64, method: &str, params: Value, session: Option<&str>) -> Vec<u8> {
    let mut message = json!({ "id": id, "method": method, "params": params });
    if let Some(session) = session {
        message["sessionId"] = json!(session);
    }
    message.to_string().into_bytes()
}

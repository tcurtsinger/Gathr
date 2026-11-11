//! Short app-server conversations for sign-in, model lists and a copy's probe.
use super::{command, rpc::Session};
use crate::claude::engine::{Engine, Line};
use crate::claude::frames::Frame;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct Client {
    pub engine: Engine,
    lines: mpsc::UnboundedReceiver<Line>,
    session: Session,
    pub version: String,
    last_error: String,
}

impl Client {
    pub async fn start(program: &Path, folder: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let env = command::env(std::env::vars_os(), folder);
        let (engine, lines) = Engine::start(program, &command::args(None, false), env, folder)
            .map_err(|e| format!("Codex did not start from {}: {e}", program.display()))?;
        let (session, first) = Session::handshake();
        let mut client = Self {
            engine,
            lines,
            session,
            version: String::new(),
            last_error: String::new(),
        };
        let initialized = async {
            client
                .engine
                .write(&first)
                .await
                .map_err(|e| e.to_string())?;
            while client.session.version.is_none() {
                client.next().await?;
            }
            Ok::<_, String>(client.session.version.clone().unwrap())
        };
        match tokio::time::timeout(Duration::from_secs(30), initialized).await {
            Ok(Ok(version)) => {
                client.version = version;
                Ok(client)
            }
            answer => {
                // No login request was sent: closing a failed handshake cannot interrupt renewal.
                client.engine.close(Duration::from_secs(10)).await;
                Err(answer
                    .unwrap_or_else(|_| Err("Codex didn't initialize in 30 s".into()))
                    .unwrap_err())
            }
        }
    }

    pub async fn next(&mut self) -> Result<Value, String> {
        while let Some(line) = self.lines.recv().await {
            let text = match line {
                Line::Err(text) => {
                    self.last_error = crate::redact::redact(&text);
                    continue;
                }
                Line::Out(text) => text,
            };
            let value: Value =
                serde_json::from_str(&text).map_err(|e| format!("Codex sent invalid JSON: {e}"))?;
            let read = self.session.read(&value);
            for send in read.send {
                self.engine.write(&send).await.map_err(|e| e.to_string())?;
            }
            for frame in read.frames {
                if let Frame::EngineError { message, .. } = frame {
                    return Err(message);
                }
            }
            return Ok(value);
        }
        Err(format!(
            "Codex exited without answering: {}",
            self.last_error
        ))
    }

    pub async fn send(&mut self, method: &str, params: Value) -> Result<u64, String> {
        let (id, line) = self.session.ask(method, params);
        self.engine.write(&line).await.map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.send(method, params).await?;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let value = self.next().await?;
                if value["id"] != id {
                    continue;
                }
                if let Some(error) = value.get("error").filter(|v| !v.is_null()) {
                    return Err(format!(
                        "{method} failed: {}",
                        error["message"].as_str().unwrap_or("unknown error")
                    ));
                }
                return Ok(value["result"].clone());
            }
        })
        .await
        .unwrap_or_else(|_| Err(format!("Codex didn't answer {method} in 30 s")))
    }
}

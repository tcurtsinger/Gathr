//! A stand-in release feed on 127.0.0.1: serves the files put in it, and counts what was asked.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Feed {
    pub port: u16,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub asked: Arc<Mutex<Vec<String>>>,
}

impl Feed {
    pub fn start() -> Feed {
        Self::serve(false)
    }

    /// A feed that takes each request and never answers it.
    pub fn silent() -> Feed {
        Self::serve(true)
    }

    fn serve(silent: bool) -> Feed {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let files: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let (f, a) = (files.clone(), asked.clone());
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                if silent {
                    a.lock().unwrap().push(String::new());
                    held.push(stream);
                    continue;
                }
                let mut line = String::new();
                let mut reader = BufReader::new(&stream);
                let _ = reader.read_line(&mut line);
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {}
                a.lock().unwrap().push(path.clone());
                let body = f.lock().unwrap().get(&path).cloned();
                let mut out = &stream;
                let _ = match body {
                    Some(body) => out
                        .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes())
                        .and_then(|()| out.write_all(&body)),
                    None => out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                };
            }
        });
        Feed { port, files, asked }
    }

    /// The feed's address: its files are under `/releases`.
    pub fn url(&self) -> PathBuf {
        PathBuf::from(format!("http://127.0.0.1:{}/releases", self.port))
    }

    /// Serves `bytes` at `/releases/<path>`.
    pub fn put(&self, path: &str, bytes: Vec<u8>) {
        self.files
            .lock()
            .unwrap()
            .insert(format!("/releases/{path}"), bytes);
    }

    /// How many requests asked for a path ending in `end`.
    pub fn asked_for(&self, end: &str) -> usize {
        let asked = self.asked.lock().unwrap();
        asked.iter().filter(|p| p.ends_with(end)).count()
    }
}

//! Where a stopped service's hand-over to its update got to (0032). Written at each step, so one
//! that stops part way says where: to the window while the old service still holds the folder,
//! and to the next start as `gathr.app.handover`.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Copies;
use crate::{ids, platform};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandOver {
    pub from: String,
    pub to: String,
    pub pid: u32,
    /// The step it's at, as the window shows it.
    pub step: String,
    pub since_us: i64,
    /// Work the stopped service still had after its wait, left to end with the process.
    #[serde(default)]
    pub left_running: bool,
}

/// The last step: the new copy is starting, and the old one exits once it answers.
pub const STARTING: &str = "starting the update";

impl HandOver {
    pub fn new(from: &str, to: &str) -> Self {
        HandOver {
            from: from.to_string(),
            to: to.to_string(),
            pid: std::process::id(),
            step: String::new(),
            since_us: 0,
            left_running: false,
        }
    }
}

impl Copies {
    fn hand_over_file(&self) -> PathBuf {
        self.root().join("handover.json")
    }

    /// Notes the step the hand-over is at, before it's taken.
    pub fn hand_over_at(&self, hand_over: &mut HandOver, step: &str) -> io::Result<()> {
        hand_over.step = step.to_string();
        hand_over.since_us = ids::now_us();
        std::fs::create_dir_all(self.root())?;
        let bytes = serde_json::to_vec(hand_over).expect("it serializes");
        platform::write_private(&self.hand_over_file(), &bytes)
    }

    /// The hand-over a service with this `pid` is in, if it's stopped for one.
    pub fn hand_over_by(&self, pid: u32) -> Option<HandOver> {
        let bytes = std::fs::read(self.hand_over_file()).ok()?;
        serde_json::from_slice::<HandOver>(&bytes)
            .ok()
            .filter(|h| h.pid == pid)
    }

    /// The last hand-over's notes, taken once by the start after it.
    pub fn take_hand_over(&self) -> Option<HandOver> {
        let bytes = std::fs::read(self.hand_over_file()).ok()?;
        let _ = std::fs::remove_file(self.hand_over_file());
        serde_json::from_slice(&bytes).ok()
    }
}

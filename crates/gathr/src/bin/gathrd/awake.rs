//! Settings > General's Keep awake: the system is kept awake while any chat works (a turn runs or a
//! command an agent left running in the background), and let go once none does.

use gathr::awake::Hold;
use gathr::event::Event;
use gathr::writer::Writer;
use tokio::sync::watch;

pub struct KeepAwake {
    working: watch::Sender<bool>,
    on: watch::Sender<bool>,
}

impl KeepAwake {
    pub fn new(on: bool) -> KeepAwake {
        KeepAwake {
            working: watch::channel(false).0,
            on: watch::channel(on).0,
        }
    }

    /// Whether any chat works now.
    pub fn working(&self, working: bool) {
        self.working
            .send_if_modified(|was| std::mem::replace(was, working) != working);
    }

    /// The person turned Keep awake on or off.
    pub fn set_on(&self, on: bool) {
        self.on
            .send_if_modified(|was| std::mem::replace(was, on) != on);
    }

    /// Holds the system awake while both say so, for as long as the service runs.
    pub async fn run(&self, writer: Writer) {
        let (mut working, mut on) = (self.working.subscribe(), self.on.subscribe());
        let mut hold: Option<Hold> = None;
        loop {
            let wanted = *working.borrow_and_update() && *on.borrow_and_update();
            match (wanted, hold.take()) {
                (true, None) => {
                    let event = Event::point("gathr.awake.hold");
                    match Hold::new() {
                        Ok(h) => {
                            hold = Some(h);
                            writer.record(event.ok());
                        }
                        Err(why) => writer.record(event.error("hold_failed").attr("why", why)),
                    }
                }
                (false, Some(mut h)) => {
                    let event = Event::point("gathr.awake.release");
                    writer.record(match h.stopped() {
                        Some(why) => event.error("helper_stopped").attr("why", why),
                        None => event.ok(),
                    });
                }
                (_, kept) => hold = kept,
            }
            tokio::select! {
                changed = working.changed() => if changed.is_err() { return },
                changed = on.changed() => if changed.is_err() { return },
            }
        }
    }
}

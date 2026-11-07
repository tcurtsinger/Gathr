//! The service's own warnings, errors and panics, into the event record (0019).

use std::fmt::Write as _;

use gathr::crash_log::CrashLog;
use gathr::event::Event;
use gathr::writer::Writer;
use tracing::field::{Field, Visit};
use tracing::{Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

pub fn install(writer: Writer, crash_log: CrashLog) {
    let layer = RecordLayer {
        writer: writer.clone(),
    };
    // A second install in the same process (never in gathrd) would only keep the first.
    let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer));

    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let message = info.payload_as_str().unwrap_or("(no message)").to_string();
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        // The writer may be what panicked, so the crash log gets it too.
        crash_log.append(&format!("panic in {thread} at {location}: {message}"));
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        writer.record(
            Event::point("gathr.service.panic")
                .error("panic")
                .attr("thread", thread)
                .attr("location", location)
                .attr("message", message)
                .attr("backtrace", backtrace),
        );
    }));
}

/// Writes every `warn` and `error` as a `gathr.log` event.
struct RecordLayer {
    writer: Writer,
}

impl<S: Subscriber> Layer<S> for RecordLayer {
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        *metadata.level() <= Level::WARN
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut row = Event::point("gathr.log")
            .attr("level", level.as_str())
            .attr("target", event.metadata().target())
            .attr("message", fields.message)
            .attr("fields", fields.rest);
        if level == Level::ERROR {
            row = row.error("log");
        }
        self.writer.record(row);
    }
}

#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, "{}={value:?} ", field.name());
        }
    }
}

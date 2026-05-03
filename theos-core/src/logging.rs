//! Shared logging setup for all Theos binaries
use std::io::Write;
use std::{fmt, io};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::time::{FormatTime, UtcTime};
use tracing_subscriber::fmt::{format, FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{fmt as tracing_fmt, EnvFilter, Registry};

/// Custom log formatter used to log in all binaries of the Theos engine
struct TheosFormatter {
    /// Internal timer used to get the current time for a log
    timer: UtcTime<time::format_description::well_known::Rfc3339>,
}

impl TheosFormatter {
    /// Initialization function to set the timer
    fn new() -> Self {
        Self {
            timer: UtcTime::rfc_3339(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for TheosFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    /// Formats a log event with the following format:
    /// [timestamp] [[level]] [module/target]: message fields...
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();

        // Timestamp
        if writer.has_ansi_escapes() {
            write!(writer, "\x1b[2m")?; // dim
        }
        self.timer.format_time(&mut writer)?;
        if writer.has_ansi_escapes() {
            write!(writer, "\x1b[0m")?;
        }

        write!(writer, " ")?;

        // Level
        let level = meta.level();
        if writer.has_ansi_escapes() {
            match *level {
                tracing::Level::ERROR => write!(writer, "\x1b[31m[ERROR]\x1b[0m")?, // red
                tracing::Level::WARN => write!(writer, "\x1b[33m[WARN]\x1b[0m")?,   // yellow
                tracing::Level::INFO => write!(writer, "\x1b[32m[INFO]\x1b[0m")?,   // green
                tracing::Level::DEBUG => write!(writer, "\x1b[34m[DEBUG]\x1b[0m")?, // blue
                tracing::Level::TRACE => write!(writer, "\x1b[35m[TRACE]\x1b[0m")?, // magenta
            }
        } else {
            write!(writer, "[{}]", level)?;
        }

        write!(writer, " ")?;

        // Module / target
        let source = meta.module_path().unwrap_or(meta.target());
        if writer.has_ansi_escapes() {
            write!(writer, "\x1b[36m{}\x1b[0m", source)?; // cyan
        } else {
            write!(writer, "{}", source)?;
        }

        write!(writer, ": ")?;

        let has_ansi_escapes = writer.has_ansi_escapes();

        let mut visitor = FieldVisitor {
            writer: &mut writer,
            first: true,
            ansi: has_ansi_escapes,
        };

        event.record(&mut visitor);
        writeln!(writer)
    }
}

/// Message and extra fields
struct FieldVisitor<'a, 'w> {
    /// Writer used to write the log entry
    writer: &'a mut format::Writer<'w>,
    /// Used for spacing the first extra field in the message
    first: bool,
    /// Whether ansi is currently supported by the writer used
    ansi: bool,
}

impl<'a, 'w> tracing::field::Visit for FieldVisitor<'a, 'w> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let is_message = field.name() == "message";

        if is_message {
            if self.ansi {
                let _ = write!(self.writer, "\x1b[37m"); // white
            }
            let _ = write!(self.writer, "{value:?}");
            if self.ansi {
                let _ = write!(self.writer, "\x1b[0m");
            }
            self.first = false;
            return;
        }

        // Add spacing before non-message fields
        if !self.first {
            let _ = write!(self.writer, " ");
        }
        self.first = false;

        // Format field name and value with ANSI coloring
        if self.ansi {
            // Special formatting for error fields
            let _ = match field.name() {
                "error" => write!(
                    self.writer,
                    "\x1b[31m{}\x1b[0m=\x1b[31m{value:?}\x1b[0m",
                    field.name()
                ),
                _ => write!(
                    self.writer,
                    "\x1b[33m{}\x1b[0m=\x1b[32m{value:?}\x1b[0m",
                    field.name()
                ),
            };
        } else {
            let _ = write!(self.writer, "{}={value:?}", field.name());
        }
    }
}

/// Initializes a global tracing Subscriber with correct formatting configured
pub fn init() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        // Build a combined spec that guarantees "info" is present as a default,
        // then attempt to parse it. If parsing fails for any reason, fall back to "info".
        let env_filter = match std::env::var("RUST_LOG") {
            Ok(spec) if !spec.trim().is_empty() => {
                // Prepend the safe default directive so parsing cannot produce a filter
                // that silently drops INFO messages.
                let combined = format!("{},{}", "info", spec);
                match EnvFilter::try_new(&combined) {
                    Ok(f) => f,
                    Err(err) => {
                        // Parsing failed — deterministic fallback to INFO
                        let _ = io::stderr().write_all(
                            format!(
                                "theos: warning: could not parse RUST_LOG {:?} ({}), falling back to \"info\"\n",
                                spec, err
                            )
                                .as_bytes(),
                        );
                        EnvFilter::new("info")
                    }
                }
            }
            _ => {
                // no env var set — use safe default
                EnvFilter::new("info")
            }
        };

        let fmt_layer = tracing_fmt::Layer::default()
            .event_format(TheosFormatter::new())
            .with_ansi(true)
            .with_writer(io::stdout);

        let subscriber = Registry::default().with(env_filter).with(fmt_layer);

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    });
}

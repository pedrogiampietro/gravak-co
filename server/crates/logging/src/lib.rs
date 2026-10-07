//! Logging for the Gravak servers: one coloured line per event in the
//! terminal, and the same lines without colour in a daily file under
//! `logs/`.
//!
//! ```text
//! 18:42:07.123  INFO  login    Gravak entered the world  account=test1 map=1002
//! ```
//!
//! The category is the event's `target` when it is one of [`CATEGORIES`]
//! (`tracing::info!(target: "trade", ...)`), else it is worked out from the
//! module that logged it.

use nu_ansi_term::{Color, Style};
use std::fmt;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    prelude::*,
    registry::LookupSpan,
    EnvFilter,
};

/// The categories a line can carry, with their colour in the terminal.
pub const CATEGORIES: [(&str, Color); 13] = [
    ("auth", Color::Cyan),
    ("login", Color::Green),
    ("world", Color::LightBlue),
    ("trade", Color::Yellow),
    ("pk", Color::Red),
    ("economy", Color::LightYellow),
    ("guild", Color::Purple),
    ("ai", Color::DarkGray),
    ("net", Color::Blue),
    ("db", Color::LightPurple),
    ("status", Color::LightCyan),
    ("server", Color::White),
    ("wasm", Color::DarkGray),
];

/// Keeps the file writer flushing; hold it until the server stops.
pub struct LogGuard {
    _file: tracing_appender::non_blocking::WorkerGuard,
}

/// Starts logging for `service` (`"auth"` or `"game"`). `verbosity` is the
/// `LOG_VERBOSITY` setting: 0 errors, 1 warnings, 2 information, 3 debug,
/// 4 and up everything. `RUST_LOG` still refines it per module.
pub fn init(service: &'static str, verbosity: i32) -> LogGuard {
    #[cfg(windows)]
    let colour = nu_ansi_term::enable_ansi_support().is_ok();
    #[cfg(not(windows))]
    let colour = true;

    let level = match verbosity {
        i32::MIN..=0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let filter = || {
        // Our crates at the chosen level; the libraries' chatter (SQL
        // statements, wasm compilation) only when something is wrong.
        let mut filter = EnvFilter::new(format!("warn,{level}"))
            .add_directive("sqlx=warn".parse().expect("directive"))
            .add_directive("wasmtime=warn".parse().expect("directive"))
            .add_directive("cranelift=warn".parse().expect("directive"));
        if let Ok(custom) = std::env::var("RUST_LOG") {
            for directive in custom.split(',').filter_map(|part| part.parse().ok()) {
                filter = filter.add_directive(directive);
            }
        }
        filter
    };

    let file = tracing_appender::rolling::daily("logs", format!("{service}.log"));
    let (file, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(Line { colour })
                .with_ansi(colour)
                .with_filter(filter()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(Line { colour: false })
                .with_ansi(false)
                .with_writer(file)
                .with_filter(filter()),
        )
        .init();
    LogGuard { _file: guard }
}

/// The startup box: the service, its settings and the credits.
///
/// ```text
/// ╭──────────────────────────────────────────────╮
/// │  GRAVAK · GAME SERVER                v0.1.0   │
/// ├──────────────────────────────────────────────┤
/// │  Port        5816                             │
/// ╰──────────────────────────────────────────────╯
/// ```
pub fn banner(title: &str, version: &str, rows: &[(&str, String)]) {
    #[cfg(windows)]
    let colour = nu_ansi_term::enable_ansi_support().is_ok();
    #[cfg(not(windows))]
    let colour = true;
    let paint = |style: Style, text: &str| if colour { style.paint(text).to_string() } else { text.to_owned() };
    let gold = Style::new().fg(Color::Fixed(178));
    let bold = Style::new().fg(Color::Fixed(222)).bold();
    let dim = Style::new().fg(Color::DarkGray);

    let credits = "Gravak · (c) 2026 Pedro Giampietro";
    // GPL-3.0 asks the interface to keep the license and where the code
    // comes from in sight.
    let license = "Server built on Gravak, dont copy, dont steal, dont sell, dont redistribute";
    let label_width = rows.iter().map(|(label, _)| label.chars().count()).max().unwrap_or(0);
    let content = rows
        .iter()
        .map(|(_, value)| label_width + 4 + value.chars().count())
        .chain([
            title.chars().count() + version.chars().count() + 6,
            credits.chars().count(),
            license.chars().count(),
        ])
        .max()
        .unwrap_or(40)
        .max(46);
    let line = |left: &str, fill: &str, right: &str| paint(gold, &format!("{left}{}{right}", fill.repeat(content + 4)));
    let row = |text: String, visible: usize| {
        format!(
            "{}  {}{}  {}",
            paint(gold, "│"),
            text,
            " ".repeat(content.saturating_sub(visible)),
            paint(gold, "│")
        )
    };

    println!();
    println!("{}", line("╭", "─", "╮"));
    let gap = content.saturating_sub(title.chars().count() + version.chars().count());
    println!(
        "{}",
        row(
            format!("{}{}{}", paint(bold, title), " ".repeat(gap), paint(dim, version)),
            content
        )
    );
    println!("{}", line("├", "─", "┤"));
    for (label, value) in rows {
        let padded = format!("{label:<label_width$}");
        println!(
            "{}",
            row(
                format!("{}    {}", paint(dim, &padded), value),
                label_width + 4 + value.chars().count()
            )
        );
    }
    println!("{}", line("├", "─", "┤"));
    println!("{}", row(paint(gold, credits), credits.chars().count()));
    println!("{}", row(paint(dim, license), license.chars().count()));
    println!("{}", line("╰", "─", "╯"));
    println!();
}

/// A line's category: the event's own target when it names one, else the
/// module that logged it.
fn category(target: &str) -> &'static str {
    if let Some((name, _)) = CATEGORIES.iter().find(|(name, _)| *name == target) {
        return name;
    }
    let rules: [(&str, &'static str); 16] = [
        ("game::packets::msg_connect", "login"),
        ("game::packets::msg_register", "login"),
        ("game::packets::msg_trade", "trade"),
        ("game::trade", "trade"),
        ("game::death", "pk"),
        ("game::packets::msg_interact", "world"),
        ("game::systems::monster_ai", "ai"),
        ("game::guild", "guild"),
        ("game::audit", "economy"),
        ("game::shop", "economy"),
        ("game::state", "server"),
        ("game_server", "server"),
        ("auth", "auth"),
        ("tq_network", "net"),
        ("tq_db", "db"),
        ("runtime", "wasm"),
    ];
    rules
        .iter()
        .find(|(prefix, _)| target.starts_with(prefix))
        .map_or("world", |(_, name)| name)
}

/// Formats one event as one line.
struct Line {
    colour: bool,
}

impl<S, N> FormatEvent<S, N> for Line
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(&self, _ctx: &FmtContext<'_, S, N>, mut writer: Writer<'_>, event: &Event<'_>) -> fmt::Result {
        let paint = |style: Style, text: &str| if self.colour { style.paint(text).to_string() } else { text.to_owned() };
        let time = chrono::Local::now().format("%H:%M:%S%.3f").to_string();
        let level = *event.metadata().level();
        let level_style = match level {
            Level::ERROR => Style::new().fg(Color::White).on(Color::Red).bold(),
            Level::WARN => Style::new().fg(Color::Yellow).bold(),
            Level::INFO => Style::new().fg(Color::Green),
            Level::DEBUG => Style::new().fg(Color::Blue),
            Level::TRACE => Style::new().fg(Color::Purple),
        };
        let category = category(event.metadata().target());
        let category_colour = CATEGORIES
            .iter()
            .find(|(name, _)| *name == category)
            .map_or(Color::White, |(_, colour)| *colour);

        let mut fields = Fields::default();
        event.record(&mut fields);
        let message_style = match level {
            Level::ERROR => Style::new().fg(Color::LightRed).bold(),
            Level::WARN => Style::new().fg(Color::LightYellow),
            _ => Style::new(),
        };
        write!(
            writer,
            "{}  {}  {}  {}",
            paint(Style::new().fg(Color::DarkGray), &time),
            paint(level_style, &format!("{:<5}", level.as_str())),
            paint(Style::new().fg(category_colour), &format!("{category:<7}")),
            paint(message_style, &fields.message),
        )?;
        if !fields.rest.is_empty() {
            let rest = fields
                .rest
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(" ");
            write!(writer, "  {}", paint(Style::new().fg(Color::DarkGray), &rest))?;
        }
        writeln!(writer)
    }
}

/// An event's message and its other fields, in order.
#[derive(Default)]
struct Fields {
    message: String,
    rest: Vec<(&'static str, String)>,
}

impl tracing::field::Visit for Fields {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
        } else {
            self.rest.push((field.name(), value.to_owned()));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.rest.push((field.name(), format!("{value:?}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_filed_by_category() {
        assert_eq!(category("trade"), "trade");
        assert_eq!(category("game::packets::msg_connect"), "login");
        assert_eq!(category("game::death"), "pk");
        assert_eq!(category("tq_network::actor"), "net");
        assert_eq!(category("auth::register"), "auth");
        assert_eq!(category("game::mining"), "world");
    }
}

use std::{collections::HashMap, fmt, sync::OnceLock};

use chrono::Utc;
use serde::Deserialize;
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

pub struct CompactEventFormatter;

impl<S, N> FormatEvent<S, N> for CompactEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let mut visitor = CompactVisitor::default();
        event.record(&mut visitor);

        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.6fZ");
        let level = encode_level(*metadata.level());
        let target = encode_target(metadata.target());
        let message = visitor.message.unwrap_or_default();
        let event_code = encode_event(metadata.target(), &message);

        write!(writer, "{timestamp}|{level}|{target}|{event_code}")?;

        for (field_name, field_value) in visitor.fields {
            let encoded_key = encode_field_name(&field_name);
            let encoded_value = encode_field_value(&field_name, &field_value);
            write!(writer, "|{encoded_key}={encoded_value}")?;
        }

        writeln!(writer)
    }
}

#[derive(Debug, Deserialize)]
struct LogCatalog {
    targets: HashMap<String, String>,
    events: HashMap<String, String>,
    fields: HashMap<String, String>,
    #[serde(default)]
    value_maps: HashMap<String, HashMap<String, String>>,
}

fn catalog() -> &'static LogCatalog {
    static INSTANCE: OnceLock<LogCatalog> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        serde_json::from_str(include_str!("../log_catalog.json"))
            .expect("log catalog json should be valid")
    })
}

fn encode_level(level: Level) -> char {
    match level {
        Level::ERROR => 'E',
        Level::WARN => 'W',
        Level::INFO => 'I',
        Level::DEBUG => 'D',
        Level::TRACE => 'T',
    }
}

fn encode_target(target: &str) -> String {
    catalog()
        .targets
        .get(target)
        .cloned()
        .unwrap_or_else(|| format!("~{}", escape_token(target)))
}

fn encode_event(target: &str, message: &str) -> String {
    let key = format!("{target}|{message}");
    catalog()
        .events
        .get(&key)
        .cloned()
        .unwrap_or_else(|| format!("~{}", escape_token(message)))
}

fn encode_field_name(field_name: &str) -> String {
    catalog()
        .fields
        .get(field_name)
        .cloned()
        .unwrap_or_else(|| format!("~{}", escape_token(field_name)))
}

fn encode_field_value(field_name: &str, field_value: &str) -> String {
    catalog()
        .value_maps
        .get(field_name)
        .and_then(|values| values.get(field_value))
        .cloned()
        .unwrap_or_else(|| escape_token(field_value))
}

fn escape_token(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'%' | b'|' | b'=' | b'\r' | b'\n' => {
                output.push('%');
                output.push_str(&format!("{byte:02X}"));
            }
            _ => output.push(byte as char),
        }
    }

    output
}

#[derive(Default)]
struct CompactVisitor {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl CompactVisitor {
    fn push_value(&mut self, field: &Field, value: String) {
        let normalized = normalize_rendered_value(value);

        if field.name() == "message" {
            self.message = Some(normalized);
        } else {
            self.fields.push((field.name().to_owned(), normalized));
        }
    }
}

impl Visit for CompactVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push_value(field, value.to_owned());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push_value(
            field,
            if value {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        );
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push_value(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push_value(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push_value(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push_value(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push_value(field, format!("{value:?}"));
    }
}

fn normalize_rendered_value(value: String) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].replace("\\\"", "\"")
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_event, encode_field_name, encode_field_value, encode_target};

    #[test]
    fn encodes_catalog_mapped_values() {
        assert_eq!(encode_target("anicargo_server::downloads"), "DL");
        assert_eq!(
            encode_event(
                "anicargo_server::downloads",
                "Synchronized active download execution snapshot"
            ),
            "DXS"
        );
        assert_eq!(encode_field_name("execution_id"), "x");
        assert_eq!(encode_field_value("state", "seeding"), "G");
    }

    #[test]
    fn escapes_unknown_tokens() {
        assert_eq!(encode_target("custom|target"), "~custom%7Ctarget");
        assert_eq!(encode_field_name("hello=world"), "~hello%3Dworld");
        assert_eq!(encode_field_value("custom", "a%b|c"), "a%25b%7Cc");
    }
}

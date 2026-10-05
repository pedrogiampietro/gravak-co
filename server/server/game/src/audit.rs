use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Appends a line to `logs/<name>`, next to the server's own logs.
fn append(name: &str, line: &str) {
    let _ = std::fs::create_dir_all("logs");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(format!("logs/{name}")) {
        let _ = writeln!(file, "{line}");
    }
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn item_line(
    timestamp: u64,
    action: &str,
    character_id: u32,
    item_uid: u32,
    item_type: u32,
    details: impl Display,
) -> String {
    format!(
        "[{timestamp}] action={action} character_id={character_id} item_uid={item_uid} item_type={item_type} {details}"
    )
}

pub fn item(action: &str, character_id: u32, item_uid: u32, item_type: u32, details: impl Display) {
    let line = item_line(timestamp(), action, character_id, item_uid, item_type, details);
    append("item-audit.log", &line);
}

/// Records a completed currency movement. `price` is kept for compatibility
/// with the existing audit format and represents the silver amount moved.
fn economic_line(
    timestamp: u64,
    action: &str,
    actor_id: u32,
    counterparty_id: u32,
    item_uid: u32,
    price: u32,
) -> String {
    format!(
        "[{timestamp}] action={action} actor_id={actor_id} counterparty_id={counterparty_id} item_uid={item_uid} price={price}"
    )
}

pub fn economic(action: &str, actor_id: u32, counterparty_id: u32, item_uid: u32, price: u32) {
    let line = economic_line(timestamp(), action, actor_id, counterparty_id, item_uid, price);
    append("economic-audit.log", &line);
}

#[cfg(test)]
mod tests {
    use super::{economic_line, item_line};

    #[test]
    fn preserves_machine_readable_item_fields() {
        assert_eq!(
            item_line(42, "trade_in", 7, 8001, 410001, "counterparty_id=8"),
            "[42] action=trade_in character_id=7 item_uid=8001 item_type=410001 counterparty_id=8"
        );
    }

    #[test]
    fn preserves_machine_readable_economic_fields() {
        assert_eq!(
            economic_line(42, "quest_reward", 7, 425, 0, 100),
            "[42] action=quest_reward actor_id=7 counterparty_id=425 item_uid=0 price=100"
        );
    }
}

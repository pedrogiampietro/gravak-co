CREATE TABLE IF NOT EXISTS character_chat_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    channel INTEGER NOT NULL,
    sender_name TEXT NOT NULL,
    recipient_name TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_character_chat_history_character_id_id
    ON character_chat_history(character_id, id DESC);

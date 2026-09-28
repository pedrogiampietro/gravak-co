CREATE TABLE IF NOT EXISTS warehouse_items (
    item_uid INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL CONSTRAINT fk_warehouse_character REFERENCES characters(character_id) ON DELETE CASCADE,
    item_type INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0 AND position < 40),
    durability INTEGER NOT NULL DEFAULT 0,
    max_durability INTEGER NOT NULL DEFAULT 0,
    UNIQUE (character_id, position)
);

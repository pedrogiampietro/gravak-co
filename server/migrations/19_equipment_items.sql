CREATE TABLE IF NOT EXISTS equipment_items (
    item_uid INTEGER PRIMARY KEY,
    character_id INTEGER NOT NULL CONSTRAINT fk_equipment_character REFERENCES characters(character_id) ON DELETE CASCADE,
    item_type INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 1 AND position <= 9),
    durability INTEGER NOT NULL DEFAULT 0,
    max_durability INTEGER NOT NULL DEFAULT 0,
    UNIQUE (character_id, position),
    UNIQUE (character_id, item_uid)
);

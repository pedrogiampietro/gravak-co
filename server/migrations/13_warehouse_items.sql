CREATE TABLE IF NOT EXISTS warehouse_items (
    item_uid BIGINT PRIMARY KEY DEFAULT nextval('item_uid_seq'),
    character_id BIGINT NOT NULL CONSTRAINT fk_warehouse_character REFERENCES characters(character_id) ON DELETE CASCADE,
    item_type BIGINT NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 40),
    durability BIGINT NOT NULL DEFAULT 0,
    max_durability BIGINT NOT NULL DEFAULT 0,
    UNIQUE (character_id, position)
);

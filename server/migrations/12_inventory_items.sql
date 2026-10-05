-- Items keep their uid as they move between tables, so they share one
-- sequence.
CREATE SEQUENCE IF NOT EXISTS item_uid_seq;

CREATE TABLE IF NOT EXISTS inventory_items (
    item_uid BIGINT PRIMARY KEY DEFAULT nextval('item_uid_seq'),
    character_id BIGINT NOT NULL CONSTRAINT fk_inventory_character REFERENCES characters(character_id) ON DELETE CASCADE,
    item_type BIGINT NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 40),
    durability BIGINT NOT NULL DEFAULT 0,
    max_durability BIGINT NOT NULL DEFAULT 0,
    UNIQUE (character_id, position)
);

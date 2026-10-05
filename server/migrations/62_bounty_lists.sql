-- The Task Board's lists (after Tibia's): monsters a character prefers come up
-- more often in its offers, and the ones it avoids never do. Each name is
-- bought with bounty points.
CREATE TABLE IF NOT EXISTS bounty_lists (
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    monster_name TEXT NOT NULL,
    avoided BOOLEAN NOT NULL,
    listed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, monster_name)
);

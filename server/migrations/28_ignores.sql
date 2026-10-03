CREATE TABLE IF NOT EXISTS character_ignores (
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    ignored_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    ignored_name TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    PRIMARY KEY (character_id, ignored_id),
    CHECK (character_id <> ignored_id)
);

CREATE INDEX IF NOT EXISTS idx_character_ignores_ignored_id
    ON character_ignores(ignored_id);

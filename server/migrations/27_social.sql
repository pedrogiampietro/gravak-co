CREATE TABLE IF NOT EXISTS character_friends (
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    friend_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    friend_name TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    PRIMARY KEY (character_id, friend_id),
    CHECK (character_id <> friend_id)
);

CREATE INDEX IF NOT EXISTS idx_character_friends_friend_id
    ON character_friends(friend_id);

CREATE TABLE IF NOT EXISTS character_friends (
    character_id INTEGER NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    friend_id INTEGER NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    friend_name TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (character_id, friend_id),
    CHECK (character_id <> friend_id)
);

CREATE INDEX IF NOT EXISTS idx_character_friends_friend_id
    ON character_friends(friend_id);

CREATE TABLE IF NOT EXISTS guilds (
    guild_id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 3 AND 24),
    leader_id INTEGER NOT NULL REFERENCES characters(character_id) ON DELETE RESTRICT,
    level INTEGER NOT NULL DEFAULT 1 CHECK (level > 0),
    silver INTEGER NOT NULL DEFAULT 0 CHECK (silver >= 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS guild_members (
    guild_id INTEGER NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    character_id INTEGER NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    role INTEGER NOT NULL DEFAULT 0 CHECK (role >= 0),
    joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (guild_id, character_id),
    UNIQUE (character_id)
);

CREATE INDEX IF NOT EXISTS idx_guild_members_character_id
    ON guild_members(character_id);

CREATE TABLE IF NOT EXISTS guild_warehouse_silver (
    guild_id INTEGER PRIMARY KEY REFERENCES guilds(guild_id) ON DELETE CASCADE,
    silver INTEGER NOT NULL DEFAULT 0 CHECK (silver >= 0)
);

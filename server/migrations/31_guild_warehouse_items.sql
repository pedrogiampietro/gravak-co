CREATE TABLE IF NOT EXISTS guild_warehouse_items (
    item_uid INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    item_type INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0 AND position < 40),
    durability INTEGER NOT NULL DEFAULT 0,
    max_durability INTEGER NOT NULL DEFAULT 0,
    UNIQUE (guild_id, position)
);

CREATE INDEX IF NOT EXISTS idx_guild_warehouse_items_guild
    ON guild_warehouse_items(guild_id);

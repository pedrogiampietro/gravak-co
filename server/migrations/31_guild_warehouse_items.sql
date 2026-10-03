CREATE TABLE IF NOT EXISTS guild_warehouse_items (
    item_uid BIGINT PRIMARY KEY DEFAULT nextval('item_uid_seq'),
    guild_id BIGINT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    item_type BIGINT NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 40),
    durability BIGINT NOT NULL DEFAULT 0,
    max_durability BIGINT NOT NULL DEFAULT 0,
    UNIQUE (guild_id, position)
);

CREATE INDEX IF NOT EXISTS idx_guild_warehouse_items_guild
    ON guild_warehouse_items(guild_id);

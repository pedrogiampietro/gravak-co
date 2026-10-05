-- The last Guild War winner; its name stays on the pole.
CREATE TABLE IF NOT EXISTS guild_war (
  id BIGINT PRIMARY KEY CHECK (id = 1),
  winner_guild_id BIGINT NOT NULL,
  winner_name TEXT NOT NULL
);

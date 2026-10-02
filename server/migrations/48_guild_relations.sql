-- Guild allies and enemies declared at the GuildDirector.
CREATE TABLE IF NOT EXISTS guild_relations (
  guild_id INTEGER NOT NULL,
  other_guild_id INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('ally', 'enemy')),
  PRIMARY KEY (guild_id, other_guild_id)
);

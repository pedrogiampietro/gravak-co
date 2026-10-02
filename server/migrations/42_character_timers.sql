-- Per-character cooldowns of NPC services (UnknownMan's daily ExpBalls...).
CREATE TABLE IF NOT EXISTS character_timers (
  character_id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  until_unix INTEGER NOT NULL,
  PRIMARY KEY (character_id, kind)
);

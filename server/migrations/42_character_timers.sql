-- Per-character cooldowns of NPC services (UnknownMan's daily ExpBalls...).
CREATE TABLE IF NOT EXISTS character_timers (
  character_id BIGINT NOT NULL,
  kind TEXT NOT NULL,
  until_unix BIGINT NOT NULL,
  PRIMARY KEY (character_id, kind)
);

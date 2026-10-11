-- The idle hunt (server/game/src/idle.rs): where a character logged out
-- hunting with the helper, and since when. On the next login the time away
-- is hunted there and the row goes.
CREATE TABLE IF NOT EXISTS character_idle (
  character_id BIGINT PRIMARY KEY,
  map_id BIGINT NOT NULL,
  x BIGINT NOT NULL,
  y BIGINT NOT NULL,
  radius BIGINT NOT NULL,
  levels_above BIGINT NOT NULL,
  since_unix BIGINT NOT NULL
);

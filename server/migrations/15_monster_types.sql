CREATE TABLE IF NOT EXISTS monster_types (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  look INTEGER NOT NULL,
  level INTEGER NOT NULL CHECK(level >= 1),
  life INTEGER NOT NULL CHECK(life >= 1),
  attack_min INTEGER NOT NULL CHECK(attack_min >= 0),
  attack_max INTEGER NOT NULL CHECK(attack_max >= attack_min),
  defense INTEGER NOT NULL CHECK(defense >= 0),
  attack_speed INTEGER NOT NULL CHECK(attack_speed >= 0),
  respawn_seconds INTEGER NOT NULL CHECK(respawn_seconds >= 0)
);
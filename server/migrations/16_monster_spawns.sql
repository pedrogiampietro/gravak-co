CREATE TABLE IF NOT EXISTS monster_spawns (
  id INTEGER PRIMARY KEY,
  monster_type_id INTEGER NOT NULL CONSTRAINT fk_monster_spawn_type REFERENCES monster_types(id) ON DELETE CASCADE,
  map_id INTEGER NOT NULL CONSTRAINT fk_monster_spawn_map REFERENCES maps(id) ON DELETE CASCADE,
  x INTEGER NOT NULL CHECK(x >= 0),
  y INTEGER NOT NULL CHECK(y >= 0),
  amount INTEGER NOT NULL DEFAULT 1 CHECK(amount >= 1),
  radius INTEGER NOT NULL DEFAULT 0 CHECK(radius >= 0)
);
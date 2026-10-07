-- Weapon skins: the look of another 5065 weapon worn over an owned one,
-- by item uid like the dyes (Tinter). The weapon keeps its own powers.
CREATE TABLE IF NOT EXISTS item_skins (
  item_uid BIGINT PRIMARY KEY,
  look BIGINT NOT NULL CHECK (look > 0)
);

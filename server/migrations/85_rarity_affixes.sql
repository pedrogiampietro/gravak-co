-- Rarity affixes, rolled and kept by the server: each rarity slot of an
-- item holds one (packed: the kind in the top four bits, the value in the
-- low twelve). Items that already have a rarity get theirs rolled when
-- their owner next logs in.
CREATE TABLE IF NOT EXISTS item_affixes (
  item_uid BIGINT NOT NULL,
  slot SMALLINT NOT NULL CHECK (slot BETWEEN 0 AND 3),
  affix INTEGER NOT NULL CHECK (affix > 0),
  PRIMARY KEY (item_uid, slot)
);

-- Failed rarity attempts on an item since it last rose: each adds to the
-- chance of the next.
CREATE TABLE IF NOT EXISTS item_rarity_failures (
  item_uid BIGINT PRIMARY KEY,
  failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0)
);

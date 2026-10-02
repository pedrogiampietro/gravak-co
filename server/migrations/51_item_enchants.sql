-- WuxingOven HP enchantments, by item uid like the blessings.
CREATE TABLE IF NOT EXISTS item_enchants (
  item_uid INTEGER PRIMARY KEY,
  hp INTEGER NOT NULL CHECK (hp BETWEEN 0 AND 255)
);

-- Item blessings (-1%, -3%, -5%, -7% damage taken), by item uid so they
-- survive warehouse moves that rewrite item_upgrades.
CREATE TABLE IF NOT EXISTS item_blessings (
  item_uid INTEGER PRIMARY KEY,
  bless INTEGER NOT NULL CHECK (bless BETWEEN 0 AND 7)
);

-- Ethereal, who strengthens blessings in the Market (Redux [1234]; the
-- reference servers do not place it, so it stands beside the Artisans).
INSERT OR IGNORE INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
VALUES (1234, 'Ethereal', 2, 1176, 1036, 186, 216, 0, 0, 0, 0, 0, 0);

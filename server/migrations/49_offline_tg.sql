-- Offline training: the OfflineTGAdmin in the training ground (Redux
-- [601]) and, since this client has no Heaven's Blessing window, a gate
-- keeper beside Twin City's Conductress who sends blessed players there.
INSERT OR IGNORE INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
VALUES
  (601, 'OfflineTGAdmin', 2, 3016, 601, 56, 55, 0, 0, 0, 0, 0, 0),
  (602, 'TrainingGuide', 2, 3016, 1002, 440, 440, 0, 0, 0, 0, 0, 0);

-- Labyrinth bosses, one per floor. The reference servers carry the
-- monster types but no spawns: each boss guards the middle of its floor,
-- returns once an hour and always drops that floor's token (the fourth
-- floor's a CloudDiamond), with DragonBalls and Meteors on top.
UPDATE monster_types SET respawn_seconds = 3600 WHERE id IN (3143, 3146, 3149, 3156);

INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (3143, 1351, 240, 168, 1, 0),
  (3146, 1352, 360, 288, 1, 0),
  (3149, 1353, 456, 360, 1, 0),
  (3156, 1354, 432, 384, 1, 0);

INSERT INTO monster_drops (monster_type_id, item_id, chance_percent)
VALUES
  (3143, 721537, 100),
  (3146, 721538, 100),
  (3149, 721539, 100),
  (3156, 721536, 100),
  (3143, 1088000, 30),
  (3146, 1088000, 40),
  (3149, 1088000, 50),
  (3156, 1088000, 60),
  (3143, 1088001, 100),
  (3146, 1088001, 100),
  (3149, 1088001, 100),
  (3156, 1088001, 100)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

DELETE FROM monster_spawns WHERE monster_type_id = 900001;
DELETE FROM monster_types WHERE id = 900001;

INSERT INTO monster_types (
  id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds
)
VALUES (1, 'Pheasant', 104, 1, 33, 5, 6, 0, 500, 10)
ON CONFLICT DO NOTHING;

INSERT INTO monster_spawns (
  id, monster_type_id, map_id, x, y, amount, radius
)
VALUES (2, 1, 1002, 384, 458, 5, 8)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

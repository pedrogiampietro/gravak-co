INSERT INTO monster_types (
  id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds
)
VALUES (900001, 'TestMonster', 900, 1, 100, 5, 8, 1, 1500, 20)
ON CONFLICT DO NOTHING;

INSERT INTO monster_spawns (
  id, monster_type_id, map_id, x, y, amount, radius
)
VALUES (1, 900001, 1002, 442, 380, 1, 0)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

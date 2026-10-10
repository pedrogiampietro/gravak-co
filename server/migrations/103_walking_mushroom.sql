-- The Covil dos Goblins' fourth creature (server/game/src/rifts.rs, tier 3):
-- the walking mushroom, a model made in Meshy (content/monsters.toml, look
-- 944). Slow and tough where the goblins are quick and frail.
INSERT INTO monster_types
    (id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds,
     view_range, attack_range, attack_mode, magic_resistance, dodge, accuracy, magic_skill)
VALUES
    (9143, 'CogumeloAndante', 944, 33, 1400, 100, 130, 70, 1200, 0, 6, 1, 3, 5, 10, 15, 0)
ON CONFLICT (id) DO NOTHING;

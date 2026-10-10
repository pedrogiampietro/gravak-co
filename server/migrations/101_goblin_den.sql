-- The Covil dos Goblins (server/game/src/rifts.rs, tier 3): a crypt of the
-- "fungos" theme for heroes of level 30 to 50 (content/rifts.toml, 9091-9093),
-- its creatures and Guardian, eight copies of each crypt and a way out in
-- each copy. The looks (941-949) are goblin skins painted on 5065 monsters
-- (content/monsters.toml) until the creatures made in Meshy take them over.
INSERT INTO monster_types
    (id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds,
     view_range, attack_range, attack_mode, magic_resistance, dodge, accuracy, magic_skill)
VALUES
    (9140, 'GoblinSaqueador', 941, 30,   900, 105, 135,  40, 1000, 0, 6, 1, 3,  0, 25, 20, 0),
    (9141, 'HomemRato',       942, 31,  1000, 110, 145,  50, 1000, 0, 6, 1, 3,  0, 20, 20, 0),
    (9142, 'GoblinXama',      943, 32,   800, 125, 160,  30, 1000, 0, 6, 1, 3, 10, 20, 25, 0),
    (9149, 'ChefeGoblin',     949, 35, 40000, 240, 310, 150, 1000, 0, 12, 2, 3, 10, 10, 40, 0)
ON CONFLICT (id) DO NOTHING;

-- The crypts themselves (a game master's $tele) and eight copies of each:
-- map id = the crypt's id * 10 + the copy (0-7); the fallen revive at the
-- copy's own entrance (13, 13). Flags: RECORD_DISABLED, PK_DISABLED,
-- TELEPORT_DISABLED, FLY_DISABLED (4 + 8 + 64 + 512).
INSERT INTO maps (id, map_id, path, revive_point_x, revive_point_y, flags, weather, reborn_map, color)
SELECT layout, layout, 'gravak-rift-' || layout || '.cmap', 13, 13, 588, 0, 1002, 4294967295
  FROM unnest(ARRAY[9091, 9092, 9093]) AS layout
ON CONFLICT (id) DO NOTHING;
INSERT INTO maps (id, map_id, path, revive_point_x, revive_point_y, flags, weather, reborn_map, color)
SELECT layout * 10 + copy, layout, 'gravak-rift-' || layout || '.cmap', 13, 13, 588, 0, layout * 10 + copy, 4294967295
  FROM unnest(ARRAY[9091, 9092, 9093]) AS layout,
       generate_series(0, 7) AS copy
ON CONFLICT (id) DO NOTHING;

-- A way out in every copy, in the entrance room's corner (ids 3472-3495).
INSERT INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
SELECT 3472 + (crypt.position - 1) * 8 + copy, 'RiftExit', 2, 1314, crypt.layout * 10 + copy, 11, 11, 0, 0, 0, 0, 0, 0
  FROM unnest(ARRAY[9091, 9092, 9093]) WITH ORDINALITY AS crypt (layout, position),
       generate_series(0, 7) AS copy
ON CONFLICT DO NOTHING;

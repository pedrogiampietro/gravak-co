-- The Rifts (server/game/src/rifts.rs, docs/fendas-design.md): the
-- creatures and Guardians of each tier, the Rift Keeper in Twin City, eight
-- copies of every crypt (one team each) and a way out in each copy.

-- Creatures and Guardians, on the looks of the 5065's skeletons, bats,
-- ghosts and devils. Names in one word, at most 15 letters.
INSERT INTO monster_types
    (id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds,
     view_range, attack_range, attack_mode, magic_resistance, dodge, accuracy, magic_skill)
VALUES
    -- Lesser Rift (level 70)
    (9150, 'Esqueleto',       211,  75,    9000,  520,  600,  300, 1000, 0, 10, 1, 3,  0, 20, 30, 0),
    (9151, 'MorcegoCripta',   206,  74,    7500,  500,  560,  200,  800, 0, 10, 1, 3,  0, 30, 30, 0),
    (9152, 'AlmaPenada',      200,  76,    8500,  540,  620,  250, 1000, 0, 10, 1, 3, 10, 20, 30, 0),
    (9159, 'GuardiaoMenor',   197,  80,  300000,  900, 1100,  900, 1000, 0, 14, 2, 3, 20, 10, 60, 0),
    -- Greater Rift (level 100)
    (9160, 'EsqueletoAntigo', 211, 104,   26000, 1100, 1250,  700, 1000, 0, 10, 1, 3,  0, 20, 40, 0),
    (9161, 'MorcegoSangue',   148, 103,   24000, 1050, 1200,  600,  800, 0, 10, 1, 3,  0, 30, 40, 0),
    (9162, 'Espectro',        308, 105,   25000, 1150, 1300,  650, 1000, 0, 10, 1, 3, 10, 20, 40, 0),
    (9169, 'GuardiaoMaior',   397, 110,  800000, 1800, 2100, 1600, 1000, 0, 14, 2, 3, 30, 10, 70, 0),
    -- Abyssal Rift (level 120)
    (9170, 'CavaleiroAbismo', 205, 124,   42000, 1650, 1850, 1100, 1000, 0, 10, 1, 3,  0, 20, 50, 0),
    (9171, 'MorcegoAbissal',  148, 123,   38000, 1600, 1800, 1000,  800, 0, 10, 1, 3,  0, 30, 50, 0),
    (9172, 'EspectroAbissal', 200, 125,   40000, 1700, 1900, 1050, 1000, 0, 10, 1, 3, 10, 20, 50, 0),
    (9179, 'GuardiaoAbismo',  197, 130, 2000000, 2600, 3000, 2400, 1000, 0, 14, 2, 3, 40, 10, 80, 0)
ON CONFLICT (id) DO NOTHING;

-- The Rift Keeper, in Twin City near where heroes arrive.
INSERT INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
VALUES (3330, 'RiftKeeper', 2, 1314, 1002, 426, 377, 0, 0, 0, 0, 0, 0)
ON CONFLICT DO NOTHING;

-- Eight copies of each crypt: map id = the crypt's id * 10 + the copy (0-7),
-- the same .DMap; the fallen revive at the copy's own entrance.
INSERT INTO maps (id, map_id, path, revive_point_x, revive_point_y, flags, weather, reborn_map, color)
SELECT layout * 10 + copy, layout, 'gravak-rift-' || layout || '.cmap', 13, 13, 588, 0, layout * 10 + copy, 4294967295
  FROM unnest(ARRAY[9101, 9102, 9103, 9111, 9112, 9113, 9121, 9122, 9123]) AS layout,
       generate_series(0, 7) AS copy
ON CONFLICT (id) DO NOTHING;

-- A way out in every copy, in the entrance room's corner (ids 3400-3471).
INSERT INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
SELECT 3400 + (crypt.position - 1) * 8 + copy, 'RiftExit', 2, 1314, crypt.layout * 10 + copy, 11, 11, 0, 0, 0, 0, 0, 0
  FROM unnest(ARRAY[9101, 9102, 9103, 9111, 9112, 9113, 9121, 9122, 9123]) WITH ORDINALITY AS crypt (layout, position),
       generate_series(0, 7) AS copy
ON CONFLICT DO NOTHING;

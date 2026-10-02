-- Monster magic resistance (Redux `MagicDefence`, a percent taken off
-- spells, as `CalculateSkillDamage` uses it).
ALTER TABLE monster_types ADD COLUMN magic_resistance INTEGER NOT NULL DEFAULT 0;

UPDATE monster_types SET magic_resistance = 5 WHERE id IN (6015);
UPDATE monster_types SET magic_resistance = 10 WHERE id IN (3028, 8415);
UPDATE monster_types SET magic_resistance = 15 WHERE id IN (6006, 6011);
UPDATE monster_types SET magic_resistance = 20 WHERE id IN (1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410, 1411, 1412, 1413, 1414, 8416, 8417, 8418, 8419);
UPDATE monster_types SET magic_resistance = 25 WHERE id IN (6000, 6001, 6002, 6003, 6004, 6005);
UPDATE monster_types SET magic_resistance = 30 WHERE id IN (101, 3000, 3001, 3002, 3003, 3004, 3005, 3006, 3007, 3008, 3009, 3010, 3011, 3012, 3013, 3014, 3015, 3016, 3017, 3018, 3019, 3020, 3021, 3022, 3023, 3024);
UPDATE monster_types SET magic_resistance = 34 WHERE id IN (2000);
UPDATE monster_types SET magic_resistance = 35 WHERE id IN (5051);
UPDATE monster_types SET magic_resistance = 40 WHERE id IN (3033, 8500);
UPDATE monster_types SET magic_resistance = 45 WHERE id IN (2001, 2005);
UPDATE monster_types SET magic_resistance = 46 WHERE id IN (2006);
UPDATE monster_types SET magic_resistance = 50 WHERE id IN (100, 102, 2007, 2010, 3025, 3027, 3148, 3607, 8219, 9005, 9010);
UPDATE monster_types SET magic_resistance = 52 WHERE id IN (6017, 8100);
UPDATE monster_types SET magic_resistance = 53 WHERE id IN (2002, 2003, 2008, 2011);
UPDATE monster_types SET magic_resistance = 55 WHERE id IN (2009, 2012, 2013);
UPDATE monster_types SET magic_resistance = 57 WHERE id IN (2040);
UPDATE monster_types SET magic_resistance = 59 WHERE id IN (2041);
UPDATE monster_types SET magic_resistance = 60 WHERE id IN (34, 35, 36, 37, 38, 39, 40, 84, 86, 88, 90, 91, 92, 93, 7004, 7005, 8421, 8422, 9006, 9011);
UPDATE monster_types SET magic_resistance = 61 WHERE id IN (2042);
UPDATE monster_types SET magic_resistance = 62 WHERE id IN (8101);
UPDATE monster_types SET magic_resistance = 63 WHERE id IN (2043);
UPDATE monster_types SET magic_resistance = 70 WHERE id IN (21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 85, 87, 89, 103, 3560, 3561, 3563, 3564, 3579, 3582, 3583, 3584, 3585, 3586, 3603, 3611, 3615, 3619, 3623, 3627, 3631, 3644, 3645, 3646, 3706, 3707, 3708, 3709, 7000, 9007, 9008, 9012, 9013);
UPDATE monster_types SET magic_resistance = 72 WHERE id IN (8102);
UPDATE monster_types SET magic_resistance = 73 WHERE id IN (3632, 3633, 3634);
UPDATE monster_types SET magic_resistance = 75 WHERE id IN (3635, 3636, 3637, 3638, 3639, 3640, 3641, 3642, 3643);
UPDATE monster_types SET magic_resistance = 80 WHERE id IN (5058);
UPDATE monster_types SET magic_resistance = 81 WHERE id IN (6018);
UPDATE monster_types SET magic_resistance = 84 WHERE id IN (8103);
UPDATE monster_types SET magic_resistance = 86 WHERE id IN (8104);
UPDATE monster_types SET magic_resistance = 87 WHERE id IN (8105, 8106);
UPDATE monster_types SET magic_resistance = 89 WHERE id IN (8107);
UPDATE monster_types SET magic_resistance = 90 WHERE id IN (3102, 3132, 3136, 4032, 4033, 4034, 4035, 4036, 5059, 6016, 8313);
UPDATE monster_types SET magic_resistance = 91 WHERE id IN (8108, 8314, 8315, 8316);
UPDATE monster_types SET magic_resistance = 92 WHERE id IN (8109, 8312, 8317, 8318, 8319);
UPDATE monster_types SET magic_resistance = 93 WHERE id IN (8309, 8310, 8311);
UPDATE monster_types SET magic_resistance = 94 WHERE id IN (2020, 3310, 3311, 3312, 3320, 8110, 8111, 8304, 8306, 8307);
UPDATE monster_types SET magic_resistance = 95 WHERE id IN (3562, 7001, 7002, 7003, 8112, 8302, 8303, 8305, 8308);
UPDATE monster_types SET magic_resistance = 96 WHERE id IN (2021, 2030, 8113, 8114, 8300, 8301);
UPDATE monster_types SET magic_resistance = 97 WHERE id IN (2022, 2031, 8115, 8116, 8117);
UPDATE monster_types SET magic_resistance = 98 WHERE id IN (2023, 2032, 2033, 3145, 8118);
UPDATE monster_types SET magic_resistance = 99 WHERE id IN (2024, 2034, 3101, 3103, 3120, 3143, 3146, 3149, 3156, 8119);

-- SnakeKing (Redux 3102): 65000 defense leaves blows at 1 damage, so it
-- falls to magic through its 90% resistance. Redux does not place it; it
-- now holds Bird Island, fights only when attacked and returns two hours
-- after it falls. Spot, temper and drops are our choice.
UPDATE monster_types SET respawn_seconds = 7200, attack_mode = 1 WHERE id = 3102;

INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES (3102, 1015, 535, 520, 1, 0);

INSERT OR IGNORE INTO monster_drops (monster_type_id, item_id, chance_percent)
VALUES
  (3102, 1088000, 100),
  (3102, 1088001, 100),
  (3102, 700003, 20);

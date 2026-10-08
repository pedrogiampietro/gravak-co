-- Monsters that cast: the ones Redux gives a `SkillType` (its monster
-- magics 10..100, which the 5065 MagicType lacks). Each casts a player magic
-- of the same kind instead, for its look and element: 10 and 70 Thunder,
-- 30 and 40 Tornado, 50 and 60 Fire, 80 to 90 FireMeteor (the Satans), 100
-- Lightning. A caster strikes from up to `magic_range` cells; its magic
-- is its attack, against the target's magic defense.
ALTER TABLE monster_types ADD COLUMN IF NOT EXISTS magic_skill BIGINT NOT NULL DEFAULT 0;

-- Thunder (Metal)
UPDATE monster_types SET magic_skill = 1000 WHERE id IN (3132, 3136, 3145, 3632, 3634, 8100, 8101, 8102, 8103, 8104, 8105, 8106, 8107, 8108, 8109, 8110, 8111, 8112, 8113, 8114, 8115, 8116, 8117, 8118, 8119);
-- Fire (Fire)
UPDATE monster_types SET magic_skill = 1001 WHERE id IN (200, 201, 3101, 3148, 3602, 3603, 3606, 3610, 3611, 3614, 3615, 3617, 3619, 3622, 3623, 3625, 3627, 3630, 3631, 3633, 5055, 5058, 5060, 8417, 8418);
-- Tornado (Wood)
UPDATE monster_types SET magic_skill = 1002 WHERE id IN (38, 3029);
-- Lightning (Metal)
UPDATE monster_types SET magic_skill = 1010 WHERE id IN (30, 31, 76, 1400, 1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410, 1411, 1412, 1413, 1414, 3026, 3102, 3588, 3589, 3590, 6011, 7004, 7005, 8415, 8416, 8419, 8421, 8500, 9000, 9001, 9002, 9003, 9020, 9021, 9022, 9023, 9025, 9026, 9027, 9028, 11169, 11170, 11171);
-- FireMeteor (Fire)
UPDATE monster_types SET magic_skill = 1180 WHERE id IN (3635, 3644, 3645, 3646);

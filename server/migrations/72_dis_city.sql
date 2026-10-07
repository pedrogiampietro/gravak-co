-- Dis City (co.99.com "Quest - Dis City"), on the four maps of the 5065
-- client: Hell Gate (2021), Hell Hall (2022), Hell Cloister (2023) and the
-- Battle Formation (2024). The reference servers carry the demons
-- (5045..5060) but neither the spawns nor the Solar Saints of the stages;
-- the spots below come from the maps' walkable cells, the Solar Saints
-- stand where the guide puts them. The quest itself is `dis_city.rs`.

-- The Solar Saints of the stages (the one in Ape City is 3215).
INSERT INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
VALUES
  (3217, 'SolarSaint', 2, 1314, 2021, 253, 57, 0, 0, 0, 0, 0, 0),
  (3218, 'SolarSaint', 2, 1314, 2022, 215, 344, 0, 0, 0, 0, 0, 0),
  (3219, 'SolarSaint', 2, 1314, 2023, 341, 383, 0, 0, 0, 0, 0, 0),
  (3220, 'SolarSaint', 2, 1314, 2024, 147, 27, 0, 0, 0, 0, 0, 0)
ON CONFLICT DO NOTHING;

-- Hell Gate: UndeadSoldier, Phantom, UndeadSpearman and Revenant, who
-- drop the SoulStones.
INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (5045, 2021, 216, 204, 5, 8), (5046, 2021, 276, 204, 5, 8), (5049, 2021, 332, 228, 5, 8), (5050, 2021, 188, 252, 5, 8),
  (5045, 2021, 256, 252, 5, 8), (5046, 2021, 304, 276, 5, 8), (5049, 2021, 364, 280, 5, 8), (5050, 2021, 204, 300, 5, 8),
  (5045, 2021, 156, 192, 5, 8), (5046, 2021, 380, 232, 5, 8), (5049, 2021, 256, 316, 5, 8), (5050, 2021, 352, 328, 5, 8),
  (5045, 2021, 128, 288, 5, 8), (5046, 2021, 124, 240, 5, 8), (5049, 2021, 168, 348, 5, 8), (5050, 2021, 416, 344, 5, 8),
  (5045, 2021, 236, 364, 5, 8), (5046, 2021, 288, 364, 5, 8), (5049, 2021, 352, 380, 5, 8), (5050, 2021, 232, 412, 5, 8);

-- Hell Hall: Eidolon, Temptress, Centicore and Naga, the demons whose
-- defeat earns the Slayer Points.
INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (5051, 2022, 248, 268, 4, 6), (5053, 2022, 176, 260, 4, 6), (5054, 2022, 212, 260, 4, 6), (5057, 2022, 288, 240, 4, 6),
  (5051, 2022, 252, 232, 4, 6), (5053, 2022, 160, 224, 4, 6), (5054, 2022, 216, 224, 4, 6), (5057, 2022, 328, 240, 4, 6),
  (5051, 2022, 288, 204, 4, 6), (5053, 2022, 332, 204, 4, 6), (5054, 2022, 108, 224, 4, 6), (5057, 2022, 140, 188, 4, 6),
  (5051, 2022, 180, 188, 4, 6), (5053, 2022, 236, 188, 4, 6), (5054, 2022, 300, 168, 4, 6), (5057, 2022, 340, 168, 4, 6),
  (5051, 2022, 104, 160, 4, 6), (5053, 2022, 72, 204, 4, 6), (5054, 2022, 68, 168, 4, 6), (5057, 2022, 160, 152, 4, 6),
  (5051, 2022, 196, 152, 4, 6), (5053, 2022, 244, 152, 4, 6), (5054, 2022, 164, 116, 4, 6), (5057, 2022, 200, 116, 4, 6),
  (5051, 2022, 236, 116, 4, 6), (5053, 2022, 164, 80, 4, 6), (5054, 2022, 212, 80, 4, 6), (5057, 2022, 248, 80, 4, 6);

-- Hell Cloister: a Hell Wraith at each of the nine posts along the
-- cloister, in the order `dis_city::WRAITH_POSTS` asks, and HellTrolls
-- roaming between them.
INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (5055, 2023, 456, 324, 1, 2), (5055, 2023, 288, 268, 1, 2), (5055, 2023, 220, 460, 1, 2),
  (5055, 2023, 120, 352, 1, 2), (5055, 2023, 236, 204, 1, 2), (5055, 2023, 424, 176, 1, 2),
  (5055, 2023, 572, 228, 1, 2), (5055, 2023, 496, 416, 1, 2), (5055, 2023, 296, 556, 1, 2),
  (5056, 2023, 368, 212, 3, 6), (5056, 2023, 180, 260, 3, 6), (5056, 2023, 320, 108, 3, 6), (5056, 2023, 492, 220, 3, 6),
  (5056, 2023, 436, 444, 3, 6), (5056, 2023, 344, 548, 3, 6), (5056, 2023, 92, 392, 3, 6), (5056, 2023, 288, 348, 3, 6);

-- Battle Formation: the Syrens whose defeat makes up the Pluto Token, and
-- Wraiths. Ultimate Pluto is summoned by the Solar Saint.
INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (5058, 2024, 152, 72, 3, 5), (5058, 2024, 192, 84, 3, 5), (5058, 2024, 100, 88, 3, 5), (5058, 2024, 200, 132, 3, 5),
  (5058, 2024, 88, 160, 3, 5), (5058, 2024, 132, 152, 3, 5), (5058, 2024, 168, 164, 3, 5), (5058, 2024, 188, 216, 3, 5),
  (5058, 2024, 132, 232, 3, 5), (5058, 2024, 152, 264, 3, 5),
  (5060, 2024, 148, 120, 4, 5);

-- SoulStone (723085) from the Hell Gate demons, DarkHorn (790001) from the
-- Syrens, and Ultimate Pluto's StarSword (723088) with a DragonBall and a
-- Meteor.
INSERT INTO monster_drops (monster_type_id, item_id, chance_percent)
VALUES
  (5045, 723085, 20), (5046, 723085, 20), (5049, 723085, 20), (5050, 723085, 20),
  (5058, 790001, 10),
  (5059, 723088, 100), (5059, 1088000, 100), (5059, 1088001, 100)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

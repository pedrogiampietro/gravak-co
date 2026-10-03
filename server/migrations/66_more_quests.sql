-- More quests: a giver in each of the other cities, with a chain on the
-- monsters around it, and a new kind of goal: bringing items (the city's own
-- medicine) instead of defeating monsters. `target_item` set means bring
-- `required_count` of that item; the monster is then 0.
ALTER TABLE quest_definitions ADD COLUMN IF NOT EXISTS target_item BIGINT;

-- Phoenix Castle: CoachLin (30081); Ape Mountain: OldGeneral (600068);
-- Desert City: GeneralShou (30104); Bird Island: GossiperWang (600062).
-- Meteor 1088001, DragonBall 1088000; Painkiller 1000020, Amrita 1000030,
-- Panacea 1002000, Ginseng 1002010.
INSERT INTO quest_definitions (
    quest_id, npc_id, name, description, target_monster_type, required_count,
    reward_silver, reward_experience, required_quest, min_level, reward_item, target_item
)
VALUES
    (10, 30081, 'Winged Pests', 'WingedSnakes swarm the roads around Phoenix Castle. Defeat twenty-five of them.', 6, 25, 3000, 7300, NULL, 25, NULL, NULL),
    (11, 30081, 'Bandit Roads', 'Bandits rob the travellers on the castle roads. Defeat thirty-five Bandits.', 7, 35, 5000, 11900, 10, 30, NULL, NULL),
    (12, 30081, 'Medicine for the Wounded', 'The guards hurt by the Bandits need medicine. Bring me five Painkillers.', 0, 5, 6000, 5900, 11, 30, NULL, 1000020),
    (13, 30081, 'The Ratling Nest', 'Ratlings breed in the hills north of the castle. Defeat thirty-five of them.', 8, 35, 8000, 18400, 12, 35, 1088001, NULL),
    (14, 30081, 'Fire in the Hills', 'FireSpirits burn the farms of the castle. Defeat thirty of them.', 9, 30, 12000, 27600, 13, 40, 1088000, NULL),
    (20, 600068, 'Macaque Mischief', 'Macaques steal from the miners of Ape Mountain. Defeat thirty-five of them.', 10, 35, 15000, 55000, NULL, 45, NULL, NULL),
    (21, 600068, 'Giant Trouble', 'GiantApes block the mountain paths. Defeat thirty-five of them.', 11, 35, 20000, 81500, 20, 50, NULL, NULL),
    (22, 600068, 'Supplies for the Camp', 'My soldiers are worn out. Bring me five Amritas.', 0, 5, 25000, 40700, 21, 50, NULL, 1000030),
    (23, 600068, 'Thunder in the Peaks', 'ThunderApes rule the high peaks. Defeat thirty of them.', 12, 30, 30000, 124000, 22, 55, 1088001, NULL),
    (24, 600068, 'The Snakemen', 'Snakemen gather west of the mountain. Defeat thirty-five of them.', 13, 35, 40000, 183300, 23, 60, 1088000, NULL),
    (30, 30104, 'Sand in the Wind', 'SandMonsters rise from the dunes of the desert. Defeat thirty-five of them.', 14, 35, 50000, 292800, NULL, 65, NULL, NULL),
    (31, 30104, 'Hill Stalkers', 'HillMonsters stalk the caravans. Defeat thirty-five of them.', 15, 35, 60000, 412700, 30, 70, NULL, NULL),
    (32, 30104, 'Water and Medicine', 'The caravans need medicine to cross the desert. Bring me five Panaceas.', 0, 5, 70000, 206300, 31, 70, NULL, 1002000),
    (33, 30104, 'Living Rocks', 'RockMonsters crush whoever walks among them. Defeat thirty of them.', 16, 30, 80000, 673200, 32, 75, 1088001, NULL),
    (34, 30104, 'The Blade Ghosts', 'BladeGhosts haunt the ruins of the desert. Defeat thirty of them.', 17, 30, 100000, 953800, 33, 80, 1088000, NULL),
    (40, 600062, 'Birdmen Rumours', 'They say Birdmen gather in the woods of the island. Defeat thirty-five of them.', 18, 35, 120000, 1429200, NULL, 85, NULL, NULL),
    (41, 600062, 'Hunting the HawKing', 'HawKings attack the island''s fishermen. Defeat thirty of them.', 19, 30, 150000, 2023400, 40, 90, NULL, NULL),
    (42, 600062, 'Ginseng for Wang', 'Gossip is tiring work. Bring me five Ginsengs.', 0, 5, 160000, 1011700, 41, 90, NULL, 1002010),
    (43, 600062, 'The Island Bandits', 'Bandits have taken the island''s south shore. Defeat thirty of them.', 55, 30, 200000, 2935500, 42, 95, 1088000, NULL)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

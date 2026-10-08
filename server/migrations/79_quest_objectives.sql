-- Two more kinds of quest goal, and the chains past level 95.
--
-- `target_npc` set: a message. The quest is done (and paid) when the
-- character speaks to that NPC; nothing is hunted or brought.
-- `drop_chance` above 0 with a `target_item` and a `target_monster_type`: a
-- collection. While the quest is under way, each kill of that monster puts
-- one `target_item` in the bag `drop_chance` times in a hundred, until the
-- character has `required_count`; they are then brought to the giver as a
-- delivery is.
ALTER TABLE quest_definitions ADD COLUMN IF NOT EXISTS target_npc BIGINT;
ALTER TABLE quest_definitions ADD COLUMN IF NOT EXISTS drop_chance INTEGER NOT NULL DEFAULT 0;

-- VillageHead (30102), Desert City: the Mystic Castle, levels 95 to 110.
-- Mike (30164), Desert City: the labyrinth and the castle's depths, 110 to 125.
-- TombBat 20, BloodyBat 56, BullMonster 57, RedDevil 58/83, Slinger 11171,
-- Bladeling 3144, FiendBat 3148, Minotaur 3155. StoneBandit 30103 (Desert
-- City), Simon 1152 (Twin City). BatWing 722700, DevilHorn 722701,
-- FiendFang 722702 (content/items/quest.toml). Meteor 1088001, DragonBall
-- 1088000.
INSERT INTO quest_definitions
    (quest_id, npc_id, name, description, target_monster_type, required_count, reward_silver, reward_experience,
     required_quest, min_level, reward_item, target_item, target_npc, drop_chance)
VALUES
    (50, 30102, 'Word from the Desert', 'The StoneBandit at the desert''s edge owes me an answer. Go and speak with him.',
     0, 1, 150000, 1500000, NULL, 95, NULL, NULL, 30103, 0),
    (51, 30102, 'The Tomb Bats', 'TombBats swarm out of the Mystic Castle at night. Defeat forty of them.',
     20, 40, 200000, 3500000, 50, 97, NULL, NULL, NULL, 0),
    (52, 30102, 'Wings for the Healer', 'Our healer brews a cure from BloodyBat wings. Bring me ten BatWings.',
     56, 10, 220000, 3000000, 51, 100, 1088001, 722700, NULL, 35),
    (53, 30102, 'The Bulls of the Castle', 'BullMonsters guard the castle''s inner halls. Defeat forty of them.',
     57, 40, 300000, 5000000, 52, 105, NULL, NULL, NULL, 0),
    (54, 30102, 'The Red Devils', 'Red Devils lead the castle''s beasts. Defeat forty of them.',
     58, 40, 400000, 6500000, 53, 108, 1088000, NULL, NULL, 0),
    (60, 30164, 'Horns of the Devil', 'A collector pays well for DevilHorns. Bring me ten, from the Red Devils of the castle.',
     83, 10, 450000, 6000000, NULL, 110, NULL, 722701, NULL, 35),
    (61, 30164, 'The Labyrinth Keeper', 'Simon in Twin City guards the way into the labyrinth. Tell him I sent you.',
     0, 1, 300000, 3000000, 60, 112, NULL, NULL, 1152, 0),
    (62, 30164, 'Slingers of the First Floor', 'Slingers hold the labyrinth''s first floor. Defeat thirty of them.',
     11171, 30, 500000, 8000000, 61, 113, NULL, NULL, NULL, 0),
    (63, 30164, 'Bladelings Below', 'Bladelings haunt the second floor. Defeat thirty of them.',
     3144, 30, 550000, 9000000, 62, 116, 1088001, NULL, NULL, 0),
    (64, 30164, 'Fangs of the Fiends', 'FiendBats nest on the third floor. Bring me ten FiendFangs.',
     3148, 10, 600000, 9500000, 63, 118, NULL, 722702, NULL, 30),
    (65, 30164, 'The Minotaurs', 'Minotaurs rule the labyrinth''s depths. Defeat thirty of them.',
     3155, 30, 800000, 12000000, 64, 120, 1088000, NULL, NULL, 0)
ON CONFLICT (quest_id) DO NOTHING;

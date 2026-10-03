-- Quests come in chains: an NPC offers the next quest a character may take,
-- once the one before it is done and the character is strong enough. A
-- reward item is given on completion, which waits until the bag has room.
ALTER TABLE quest_definitions
    ADD COLUMN required_quest BIGINT REFERENCES quest_definitions(quest_id),
    ADD COLUMN min_level BIGINT NOT NULL DEFAULT 1 CHECK (min_level >= 1),
    ADD COLUMN reward_item BIGINT;

-- Old General Yang's training, across the monsters around Twin City
-- (Meteor 1088001, DragonBall 1088000).
INSERT INTO quest_definitions (
    quest_id, npc_id, name, description, target_monster_type, required_count,
    reward_silver, reward_experience, required_quest, min_level, reward_item
)
VALUES
    (2, 425, 'Turtledove Trouble', 'Turtledoves raid the fields east of Twin City. Defeat ten of them.', 2, 10, 500, 500, 1, 5, NULL),
    (3, 425, 'Robin Raid', 'Robins nest south of the city. Defeat fifteen Robins.', 3, 15, 1200, 5000, 2, 10, 1088001),
    (4, 425, 'Restless Spirits', 'Apparitions haunt the southwest. Defeat twenty of them.', 4, 20, 2500, 12000, 3, 15, NULL),
    (5, 425, 'The Poltergeist Woods', 'Poltergeists gather in the western woods. Defeat twenty-five of them.', 5, 25, 5000, 25000, 4, 20, 1088001),
    (6, 425, 'The WarGhost', 'A WarGhost leads the spirits of the southwest. Defeat it.', 24, 1, 8000, 30000, 5, 20, 1088000)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();

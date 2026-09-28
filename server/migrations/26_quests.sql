CREATE TABLE IF NOT EXISTS quest_definitions (
    quest_id INTEGER PRIMARY KEY,
    npc_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    target_monster_type INTEGER NOT NULL,
    required_count INTEGER NOT NULL CHECK (required_count > 0),
    reward_silver INTEGER NOT NULL DEFAULT 0 CHECK (reward_silver >= 0),
    reward_experience INTEGER NOT NULL DEFAULT 0 CHECK (reward_experience >= 0)
);

CREATE TABLE IF NOT EXISTS character_quests (
    character_id INTEGER NOT NULL CONSTRAINT fk_character_quest_character REFERENCES characters(character_id) ON DELETE CASCADE,
    quest_id INTEGER NOT NULL CONSTRAINT fk_character_quest_definition REFERENCES quest_definitions(quest_id) ON DELETE CASCADE,
    progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0),
    state INTEGER NOT NULL DEFAULT 1 CHECK (state IN (1, 2)),
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    completed_at INTEGER,
    PRIMARY KEY (character_id, quest_id)
);

INSERT OR IGNORE INTO quest_definitions (
    quest_id,
    npc_id,
    name,
    description,
    target_monster_type,
    required_count,
    reward_silver,
    reward_experience
)
VALUES (
    1,
    425,
    'Pheasant Hunt',
    'Defeat one Pheasant near the training grounds.',
    1,
    1,
    100,
    50
);

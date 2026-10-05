-- Bounty tasks (after Tibia's): pick one of three monsters of a difficulty,
-- defeat enough of them, and earn experience, bounty points and a reroll
-- token. Points upgrade the talisman, whose bonuses work against the monster
-- of the task under way.

CREATE TABLE IF NOT EXISTS bounty_profiles (
    character_id BIGINT PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    points BIGINT NOT NULL DEFAULT 0 CHECK (points >= 0),
    reroll_tokens BIGINT NOT NULL DEFAULT 3 CHECK (reroll_tokens BETWEEN 0 AND 10),
    -- The day (days since 1970, UTC) the daily token was last claimed.
    daily_claimed_on BIGINT NOT NULL DEFAULT 0,
    -- 0 Beginner, 1 Adept, 2 Expert, 3 Master.
    difficulty BIGINT NOT NULL DEFAULT 0 CHECK (difficulty BETWEEN 0 AND 3),
    -- Talisman levels: damage, life leech, loot, double kills.
    damage_level BIGINT NOT NULL DEFAULT 0 CHECK (damage_level >= 0),
    leech_level BIGINT NOT NULL DEFAULT 0 CHECK (leech_level >= 0),
    loot_level BIGINT NOT NULL DEFAULT 0 CHECK (loot_level >= 0),
    double_kill_level BIGINT NOT NULL DEFAULT 0 CHECK (double_kill_level >= 0),
    completed BIGINT NOT NULL DEFAULT 0
);

-- The three monsters on offer, until one is picked.
CREATE TABLE IF NOT EXISTS bounty_offers (
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    slot BIGINT NOT NULL CHECK (slot BETWEEN 0 AND 2),
    monster_type BIGINT NOT NULL,
    kills BIGINT NOT NULL CHECK (kills > 0),
    -- 0 Normal, 1 Silver, 2 Gold.
    grade BIGINT NOT NULL CHECK (grade BETWEEN 0 AND 2),
    points BIGINT NOT NULL,
    experience BIGINT NOT NULL,
    PRIMARY KEY (character_id, slot)
);

-- The task under way; one per character.
CREATE TABLE IF NOT EXISTS bounty_tasks (
    character_id BIGINT PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    monster_type BIGINT NOT NULL,
    kills_required BIGINT NOT NULL CHECK (kills_required > 0),
    kills_done BIGINT NOT NULL DEFAULT 0 CHECK (kills_done >= 0),
    grade BIGINT NOT NULL,
    points BIGINT NOT NULL,
    experience BIGINT NOT NULL,
    started_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

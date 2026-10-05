-- Daily rewards (the Reward Shrine): each character's streak of days in a
-- row and the last day claimed (days since 1970, UTC), and what was taken.

CREATE TABLE IF NOT EXISTS daily_rewards (
    character_id BIGINT PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    streak BIGINT NOT NULL DEFAULT 0 CHECK (streak >= 0),
    last_day BIGINT NOT NULL DEFAULT -1
);

CREATE TABLE IF NOT EXISTS daily_reward_claims (
    claim_id BIGSERIAL PRIMARY KEY,
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    cycle_day BIGINT NOT NULL CHECK (cycle_day BETWEEN 1 AND 7),
    item_type BIGINT NOT NULL,
    amount BIGINT NOT NULL CHECK (amount > 0),
    claimed_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS daily_reward_claims_character ON daily_reward_claims (character_id, day);

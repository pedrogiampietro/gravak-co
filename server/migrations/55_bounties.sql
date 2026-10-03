-- Bounties: silver placed on a character's head, paid to the player who
-- slays them. One row per target; further bounties add to the reward.
CREATE TABLE IF NOT EXISTS bounties (
    target_id INTEGER PRIMARY KEY NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    reward INTEGER NOT NULL CHECK (reward > 0),
    placed_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS bounties_by_reward ON bounties (reward DESC);

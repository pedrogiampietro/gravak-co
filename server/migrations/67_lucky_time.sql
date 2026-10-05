-- Lucky time (seconds), stored by praying with Bless and worn off a second
-- at a time.
ALTER TABLE characters ADD COLUMN IF NOT EXISTS lucky_time BIGINT NOT NULL DEFAULT 0;

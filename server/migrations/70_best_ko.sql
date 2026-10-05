-- The most KOs a character has scored in one Cyclone or Superman, for
-- the highscores.
ALTER TABLE characters ADD COLUMN IF NOT EXISTS best_ko BIGINT NOT NULL DEFAULT 0 CHECK (best_ko >= 0);

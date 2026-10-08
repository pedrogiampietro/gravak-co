-- The guild's crest: its emblem (bits 0-7, game-icons.net art in the
-- client), the field's colour (bits 8-15) and the emblem's (bits 16-23).
-- 0 is a gold wolf on crimson.
ALTER TABLE guilds ADD COLUMN IF NOT EXISTS emblem BIGINT NOT NULL DEFAULT 0 CHECK (emblem >= 0);

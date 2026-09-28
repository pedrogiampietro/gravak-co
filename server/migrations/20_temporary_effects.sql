-- Persisted expiration timestamps for runtime temporary effects.
ALTER TABLE characters
    ADD COLUMN double_experience_until INTEGER NOT NULL DEFAULT 0;

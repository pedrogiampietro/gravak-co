-- Persisted expiration timestamp for the classic Heaven's Blessing effect.
ALTER TABLE characters
    ADD COLUMN heaven_blessing_until INTEGER NOT NULL DEFAULT 0;

-- Persisted expiration timestamp for the classic Heaven's Blessing effect.
ALTER TABLE characters
    ADD COLUMN heaven_blessing_until BIGINT NOT NULL DEFAULT 0;

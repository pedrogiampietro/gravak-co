ALTER TABLE characters
    ADD COLUMN nobility_donation BIGINT NOT NULL DEFAULT 0
    CHECK (nobility_donation >= 0);

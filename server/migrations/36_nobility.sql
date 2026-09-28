ALTER TABLE characters
    ADD COLUMN nobility_donation INTEGER NOT NULL DEFAULT 0
    CHECK (nobility_donation >= 0);

ALTER TABLE characters
    ADD COLUMN equipped_title INTEGER NOT NULL DEFAULT 0
    CHECK (equipped_title >= 0);

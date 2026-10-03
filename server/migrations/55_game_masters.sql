-- Game masters may use the server's `$` commands (teleport, items, silver,
-- test fixtures...). The accounts that exist when this runs are the
-- development ones, so they keep the commands; new accounts are players.
ALTER TABLE accounts ADD COLUMN gm BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE accounts SET gm = TRUE;

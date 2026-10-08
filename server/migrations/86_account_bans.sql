-- Account bans: until when (infinity for good) and why. A banned account is
-- refused at login (AccountBanned) and at the game server's door.
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS banned_until TIMESTAMPTZ;
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS ban_reason TEXT;

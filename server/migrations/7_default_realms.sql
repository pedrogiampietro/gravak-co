-- Add migration script here
INSERT INTO realms (name, game_ip_address, game_port)
VALUES (
    'CoEmu',
    '127.0.0.1',
    -- Change this to the game server's address for a remote deployment
    5816
  ) ON CONFLICT(name) DO NOTHING;

SELECT sync_identity_sequences();

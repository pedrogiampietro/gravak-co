-- The project is Gravak: the realm and the development account take its
-- name. Clients that still ask for the realm "CoEmu" are sent to it too.
UPDATE realms SET name = 'Gravak' WHERE name = 'CoEmu';
UPDATE accounts SET username = 'gravak'
WHERE username = 'shekohex' AND NOT EXISTS (SELECT 1 FROM accounts WHERE username = 'gravak');

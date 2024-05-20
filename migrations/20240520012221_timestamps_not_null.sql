-- Users table migration
UPDATE users SET created_at = NOW() WHERE created_at IS NULL;
UPDATE users SET updated_at = NOW() WHERE updated_at IS NULL;

ALTER TABLE users 
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN updated_at SET DEFAULT NOW();

-- Chats table migration
UPDATE chats SET created_at = NOW() WHERE created_at IS NULL;
UPDATE chats SET updated_at = NOW() WHERE updated_at IS NULL;

ALTER TABLE chats 
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN updated_at SET DEFAULT NOW();

-- Messages table migration
UPDATE messages SET created_at = NOW() WHERE created_at IS NULL;
UPDATE messages SET updated_at = NOW() WHERE updated_at IS NULL;

ALTER TABLE messages 
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN updated_at SET DEFAULT NOW();

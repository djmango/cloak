-- Add migration script here
ALTER TABLE memories
DROP COLUMN emoji;

-- Remove the index on the emoji column
DROP INDEX IF EXISTS idx_memories_emoji;

-- Add migration script here
-- Add migration script here
-- Remove columns "grouping" and "emoji" from memories table
ALTER TABLE memory_groups
DROP COLUMN IF EXISTS memory_id

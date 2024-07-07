-- Add migration script here
-- Remove columns "grouping" and "emoji" from memories table
ALTER TABLE memories
DROP COLUMN IF EXISTS grouping,
DROP COLUMN IF EXISTS emoji;

-- Add column group_id to memories table
ALTER TABLE memories
ADD COLUMN group_id UUID;

-- Create memory_groups table
CREATE TABLE memory_groups (
    id UUID PRIMARY KEY,
    memory_id UUID NOT NULL,
    name TEXT NOT NULL,
    emoji TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP WITH TIME ZONE,
    FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

-- Create or replace the trigger function to update the updated_at field on updates
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update the updated_at field before any update operation
CREATE TRIGGER set_timestamp
BEFORE UPDATE ON memory_groups
FOR EACH ROW
EXECUTE PROCEDURE trigger_set_timestamp();

-- Create or replace the function for soft delete to update the deleted_at field
CREATE OR REPLACE FUNCTION soft_delete_memory_group()
RETURNS TRIGGER AS $$
BEGIN
    NEW.deleted_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to handle soft deletes by setting the deleted_at field instead of actual deletion
CREATE TRIGGER set_soft_delete
BEFORE DELETE ON memory_groups
FOR EACH ROW
EXECUTE PROCEDURE soft_delete_memory_group();

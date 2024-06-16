-- Create the memories table
CREATE TABLE memories (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    content TEXT NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE DEFAULT NULL
);

-- Create or replace the trigger function to update the updated_at field
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update the updated_at field before any update operation.
CREATE TRIGGER set_timestamp
BEFORE UPDATE ON memories
FOR EACH ROW
EXECUTE PROCEDURE trigger_set_timestamp();

-- Update existing records to ensure created_at and updated_at fields are not null
UPDATE memories SET created_at = NOW() WHERE created_at IS NULL;
UPDATE memories SET updated_at = NOW() WHERE updated_at IS NULL;

-- Ensure created_at and updated_at columns are not null and have default values
ALTER TABLE memories 
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN updated_at SET DEFAULT NOW();

-- Add soft delete functionality to the memories table
-- Update the deleted_at field to the current timestamp instead of deleting the row
CREATE OR REPLACE FUNCTION soft_delete_memory()
RETURNS TRIGGER AS $$
BEGIN
    NEW.deleted_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically set the deleted_at field before delete operation.
CREATE TRIGGER set_soft_delete
BEFORE DELETE ON memories
FOR EACH ROW
EXECUTE PROCEDURE soft_delete_memory();

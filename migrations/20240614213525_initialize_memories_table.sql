-- Assuming the users table already exists with an id of type TEXT

-- Create the memories table with appropriate data types and defaults, including a foreign key constraint
CREATE TABLE memories (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    content TEXT NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
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
BEFORE UPDATE ON memories
FOR EACH ROW
EXECUTE PROCEDURE trigger_set_timestamp();

-- Ensure the created_at and updated_at columns are never null and always have default values
ALTER TABLE memories
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET NOT NULL,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Create or replace the function for soft delete to update the deleted_at field
CREATE OR REPLACE FUNCTION soft_delete_memory()
RETURNS TRIGGER AS $$
BEGIN
    NEW.deleted_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to handle soft deletes by setting the deleted_at field instead of actual deletion
CREATE TRIGGER set_soft_delete
BEFORE DELETE ON memories
FOR EACH ROW
EXECUTE PROCEDURE soft_delete_memory();

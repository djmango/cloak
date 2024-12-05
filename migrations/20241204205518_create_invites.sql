-- Create the invites table with appropriate data types and defaults, including a foreign key constraint
CREATE TABLE invites (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    email TEXT NOT NULL,
    created_at TIMESTAMP
    WITH
        TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Ensure the created_at column is never null and always have default values
ALTER TABLE invites
ALTER COLUMN created_at
SET
    NOT NULL,
ALTER COLUMN created_at
SET DEFAULT CURRENT_TIMESTAMP;

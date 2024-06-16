ALTER TABLE users
ADD COLUMN linked_to_keywords BOOLEAN DEFAULT FALSE;

-- Update the existing records to set the default value to false
UPDATE users
SET linked_to_keywords = FALSE;

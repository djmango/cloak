-- Add migration script here
ALTER TABLE memories
ADD COLUMN grouping VARCHAR(255),
ADD COLUMN emoji VARCHAR(32);

-- Update existing rows with default values
UPDATE memories
SET grouping = NULL,
    emoji = NULL
WHERE grouping IS NULL AND emoji IS NULL;

-- Add indexes for improved query performance
CREATE INDEX idx_memories_grouping ON memories (grouping);
CREATE INDEX idx_memories_emoji ON memories (emoji);

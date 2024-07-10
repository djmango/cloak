ALTER TABLE memories
ADD COLUMN grouping VARCHAR(255);

-- Add indexes for improved query performance
CREATE INDEX idx_memories_grouping ON memories (grouping);

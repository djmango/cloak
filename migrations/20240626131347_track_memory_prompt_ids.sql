-- Create table to store memory prompts and track performance
CREATE TABLE memory_prompts (
    id UUID PRIMARY KEY,
    prompt TEXT NOT NULL,
    upvotes INTEGER DEFAULT 0 NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Alter memories table to track prompt used to create memory
ALTER TABLE memories
ADD COLUMN memory_prompt_id UUID NOT NULL, -- Assumes memories table is empty
ADD CONSTRAINT fk_memory_prompt_id
FOREIGN KEY (memory_prompt_id)
REFERENCES memory_prompts(id)
ON DELETE CASCADE;

-- Alter messages table to track memories used to create response
ALTER TABLE messages
ADD COLUMN memory_ids UUID[];



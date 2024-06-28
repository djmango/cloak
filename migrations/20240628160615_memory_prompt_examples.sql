-- Add migration script here
ALTER TABLE memory_prompts
ADD COLUMN example TEXT;

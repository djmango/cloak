-- Add migration script here
ALTER TABLE messages
ADD COLUMN model_id VARCHAR(128);

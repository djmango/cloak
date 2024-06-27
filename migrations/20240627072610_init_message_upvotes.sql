-- Add migration script here
ALTER TABLE messages 
ADD COLUMN upvoted BOOLEAN DEFAULT FALSE;

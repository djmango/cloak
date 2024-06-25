-- Add migration script here
ALTER TABLE chats
ADD COLUMN parent_message_id UUID;

ALTER TABLE messages
ADD COLUMN model_id VARCHAR,

-- ALTER TABLE chats
-- ADD COLUMN parent_id UUID,
-- ADD CONSTRAINT fk_parent_message
--     FOREIGN KEY (parent_id)
--     REFERENCES messages (id);

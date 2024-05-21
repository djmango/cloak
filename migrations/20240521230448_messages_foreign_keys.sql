-- Add foreign keys to the messages table

-- Assuming chat_id references the id column of a chats table
ALTER TABLE messages
ADD CONSTRAINT fk_chat_id
FOREIGN KEY (chat_id)
REFERENCES chats(id)
ON DELETE CASCADE;

-- Assuming user_id references the id column of a users table
ALTER TABLE messages
ADD CONSTRAINT fk_user_id
FOREIGN KEY (user_id)
REFERENCES users(id)
ON DELETE CASCADE;

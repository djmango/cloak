CREATE TYPE filetype_enum AS ENUM ('jpeg', 'pdf', 'mp4', 'mp3');

CREATE TABLE files (
    id UUID PRIMARY KEY,
    message_id UUID NOT NULL,
    chat_id UUID NOT NULL,
    user_id TEXT NOT NULL,
    filetype filetype_enum NOT NULL,
    show_to_user BOOLEAN NOT NULL DEFAULT TRUE,
    url TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (message_id) REFERENCES messages(id),
    FOREIGN KEY (chat_id) REFERENCES chats(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

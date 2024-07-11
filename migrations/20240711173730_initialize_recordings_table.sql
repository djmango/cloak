-- Add migration script here
-- Add migration script here
CREATE TABLE recordings (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    s3_object_key TEXT NOT NULL,
    start_timestamp TIMESTAMP NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE
);
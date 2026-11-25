CREATE TABLE message_images (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 4),
    mime_type TEXT NOT NULL CHECK (mime_type IN ('image/png', 'image/jpeg', 'image/webp', 'image/gif')),
    data_base64 TEXT NOT NULL,
    PRIMARY KEY (message_id, ordinal)
);

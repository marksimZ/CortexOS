CREATE VIRTUAL TABLE IF NOT EXISTS chat_messages_fts USING fts5(
  message_id UNINDEXED,
  workspace_id UNINDEXED,
  conversation_id UNINDEXED,
  content_terms,
  tokenize='unicode61'
);

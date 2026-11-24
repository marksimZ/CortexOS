CREATE TABLE global_persona (
  id INTEGER PRIMARY KEY CHECK(id = 1),
  display_name TEXT NOT NULL DEFAULT '',
  role_background TEXT NOT NULL DEFAULT '',
  communication_style TEXT NOT NULL DEFAULT '',
  preferences_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(preferences_json)),
  rules_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(rules_json)),
  updated_at TEXT NOT NULL
);

CREATE TABLE memory_candidates (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  source_message_id TEXT,
  scope TEXT NOT NULL CHECK(scope IN ('global_preference', 'global_rule', 'workspace_fact')),
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'accepted', 'dismissed')),
  created_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE INDEX idx_memory_candidates_workspace_conv
  ON memory_candidates(workspace_id, conversation_id, status, created_at DESC);

CREATE TABLE conversation_summaries (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  summary TEXT NOT NULL,
  message_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_conversation_summaries_workspace
  ON conversation_summaries(workspace_id, updated_at DESC);

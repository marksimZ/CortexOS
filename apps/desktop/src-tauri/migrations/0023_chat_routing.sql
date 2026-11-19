CREATE TABLE model_chat_routing (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  config_json TEXT NOT NULL CHECK(json_valid(config_json))
);
CREATE TABLE model_backup_requests (
  run_id TEXT PRIMARY KEY REFERENCES execution_runs(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  request_json TEXT NOT NULL CHECK(json_valid(request_json)),
  request_hash TEXT NOT NULL,
  manifest_json TEXT NOT NULL CHECK(json_valid(manifest_json))
);

CREATE TRIGGER immutable_model_backup_update BEFORE UPDATE ON model_backup_requests
BEGIN SELECT RAISE(ABORT,'Backup model requests are immutable'); END;
CREATE TRIGGER immutable_model_backup_delete BEFORE DELETE ON model_backup_requests
BEGIN SELECT RAISE(ABORT,'Backup model requests are immutable'); END;

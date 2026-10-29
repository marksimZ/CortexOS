CREATE TABLE benchmark_profile_versions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  version INTEGER NOT NULL CHECK(version > 0),
  action TEXT NOT NULL CHECK(action IN ('save','skip','reset')),
  profile_json TEXT NOT NULL CHECK(json_valid(profile_json) AND length(profile_json)<=16384),
  content_hash TEXT NOT NULL CHECK(length(content_hash)=64),
  created_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id,version)
);
CREATE TRIGGER benchmark_profile_no_update BEFORE UPDATE ON benchmark_profile_versions
BEGIN SELECT RAISE(ABORT,'Profile versions are immutable'); END;
CREATE TRIGGER benchmark_profile_no_delete BEFORE DELETE ON benchmark_profile_versions
BEGIN SELECT RAISE(ABORT,'Profile versions are immutable'); END;

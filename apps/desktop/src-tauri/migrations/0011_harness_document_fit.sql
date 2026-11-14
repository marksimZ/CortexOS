CREATE TABLE harness_fit_assessments (
 workspace_id TEXT NOT NULL REFERENCES workspaces(id),
 version INTEGER NOT NULL CHECK(version>0),
 body_json TEXT NOT NULL CHECK(json_valid(body_json)),
 content_hash TEXT NOT NULL,
 created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,version)
);
CREATE TRIGGER immutable_harness_fit BEFORE UPDATE ON harness_fit_assessments
BEGIN SELECT RAISE(ABORT,'Harness document assessments are immutable'); END;
CREATE TRIGGER retain_harness_fit BEFORE DELETE ON harness_fit_assessments
BEGIN SELECT RAISE(ABORT,'Retain harness assessment evidence'); END;

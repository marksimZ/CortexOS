CREATE TABLE benchmark_repository_snapshots (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id),
 name TEXT NOT NULL, content_hash TEXT NOT NULL, profile_json TEXT NOT NULL CHECK(json_valid(profile_json)),
 files_json TEXT NOT NULL CHECK(json_valid(files_json)), created_at TEXT NOT NULL, UNIQUE(workspace_id,id)
);
CREATE TRIGGER immutable_repository_snapshot BEFORE UPDATE ON benchmark_repository_snapshots
BEGIN SELECT RAISE(ABORT,'Repository snapshots are immutable'); END;
CREATE TRIGGER keep_repository_snapshot BEFORE DELETE ON benchmark_repository_snapshots
BEGIN SELECT RAISE(ABORT,'Retain repository evidence snapshots'); END;
CREATE TABLE benchmark_generation_jobs (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id), status TEXT NOT NULL,
 plan_json TEXT NOT NULL CHECK(json_valid(plan_json)), plan_hash TEXT NOT NULL,
 response_json TEXT, usage_json TEXT, error TEXT, created_at TEXT NOT NULL, finished_at TEXT,
 UNIQUE(workspace_id,id)
);
CREATE TABLE benchmark_generated_cases (
 job_id TEXT NOT NULL REFERENCES benchmark_generation_jobs(id), case_id TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>0), candidate_json TEXT NOT NULL CHECK(json_valid(candidate_json)),
 validation_json TEXT NOT NULL CHECK(json_valid(validation_json)), created_at TEXT NOT NULL,
 PRIMARY KEY(job_id,case_id,revision)
);
CREATE TRIGGER immutable_generation_plan BEFORE UPDATE OF id,workspace_id,plan_json,plan_hash ON benchmark_generation_jobs
BEGIN SELECT RAISE(ABORT,'Generation plans are immutable'); END;
CREATE TRIGGER immutable_candidate_revision BEFORE UPDATE ON benchmark_generated_cases
BEGIN SELECT RAISE(ABORT,'Candidate revisions are immutable'); END;
CREATE TRIGGER keep_candidate_revision BEFORE DELETE ON benchmark_generated_cases
BEGIN SELECT RAISE(ABORT,'Retain candidate revisions'); END;
CREATE TABLE benchmark_dataset_reviews (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id), job_id TEXT NOT NULL,
 selection_json TEXT NOT NULL CHECK(json_valid(selection_json)), dataset_id TEXT NOT NULL, dataset_version INTEGER NOT NULL,
 reviewer TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(dataset_id,dataset_version),
 FOREIGN KEY(dataset_id,dataset_version) REFERENCES benchmark_datasets(id,version),
 FOREIGN KEY(workspace_id,job_id) REFERENCES benchmark_generation_jobs(workspace_id,id)
);
CREATE TRIGGER immutable_dataset_review BEFORE UPDATE ON benchmark_dataset_reviews
BEGIN SELECT RAISE(ABORT,'Reviews are immutable'); END;
CREATE TRIGGER keep_dataset_review BEFORE DELETE ON benchmark_dataset_reviews
BEGIN SELECT RAISE(ABORT,'Retain dataset reviews'); END;
CREATE TABLE benchmark_batches (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id), status TEXT NOT NULL,
 plan_json TEXT NOT NULL CHECK(json_valid(plan_json)), plan_hash TEXT NOT NULL,
 created_at TEXT NOT NULL, finished_at TEXT, UNIQUE(workspace_id,id)
);
CREATE TRIGGER immutable_batch_plan BEFORE UPDATE OF id,workspace_id,plan_json,plan_hash ON benchmark_batches
BEGIN SELECT RAISE(ABORT,'Batch plans are immutable'); END;
CREATE TABLE benchmark_batch_items (
 batch_id TEXT NOT NULL REFERENCES benchmark_batches(id), ordinal INTEGER NOT NULL,
 model_index INTEGER NOT NULL, case_id TEXT NOT NULL, status TEXT NOT NULL,
 run_id TEXT REFERENCES benchmark_runs(id), error TEXT, PRIMARY KEY(batch_id,ordinal)
);

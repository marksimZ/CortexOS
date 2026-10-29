ALTER TABLE conversations ADD COLUMN purpose TEXT NOT NULL DEFAULT 'chat'
  CHECK(purpose IN ('chat','benchmark'));
CREATE INDEX conversations_purpose ON conversations(workspace_id,purpose,updated_at,id);
CREATE TRIGGER immutable_conversation_purpose BEFORE UPDATE OF purpose ON conversations
WHEN NEW.purpose != OLD.purpose
BEGIN SELECT RAISE(ABORT,'Conversation purpose is immutable'); END;

CREATE TABLE benchmark_datasets (
  id TEXT NOT NULL, version INTEGER NOT NULL CHECK(version > 0), name TEXT NOT NULL,
  schema_version INTEGER NOT NULL CHECK(schema_version > 0), content_hash TEXT NOT NULL CHECK(length(content_hash)=64),
  manifest_json TEXT NOT NULL CHECK(json_valid(manifest_json) AND length(manifest_json)<=10485760),
  provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json) AND length(provenance_json)<=65536),
  created_at TEXT NOT NULL, PRIMARY KEY(id,version)
);
CREATE TABLE benchmark_cases (
  dataset_id TEXT NOT NULL, dataset_version INTEGER NOT NULL, case_id TEXT NOT NULL,
  category TEXT NOT NULL CHECK(category IN ('zh_knowledge','math_logic','coding','rag_citation','structured_tools')),
  input_json TEXT NOT NULL CHECK(json_valid(input_json) AND length(input_json)<=2097152),
  expected_json TEXT NOT NULL CHECK(json_valid(expected_json) AND length(expected_json)<=2097152),
  scoring_json TEXT NOT NULL CHECK(json_valid(scoring_json) AND length(scoring_json)<=65536),
  difficulty TEXT NOT NULL CHECK(difficulty IN ('easy','medium','hard')),
  output_format TEXT NOT NULL,
  requirements_json TEXT NOT NULL CHECK(json_valid(requirements_json) AND json_type(requirements_json)='array' AND length(requirements_json)<=65536),
  fixture_json TEXT CHECK(fixture_json IS NULL OR (json_valid(fixture_json) AND length(fixture_json)<=2097152)),
  content_hash TEXT NOT NULL CHECK(length(content_hash)=64),
  PRIMARY KEY(dataset_id,dataset_version,case_id),
  FOREIGN KEY(dataset_id,dataset_version) REFERENCES benchmark_datasets(id,version)
);
CREATE TABLE benchmark_suites (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id), name TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK(revision > 0),
  config_json TEXT NOT NULL CHECK(json_valid(config_json) AND length(config_json)<=1048576),
  updated_at TEXT NOT NULL, UNIQUE(workspace_id,id)
);
CREATE TABLE benchmark_runs (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  dataset_id TEXT NOT NULL, dataset_version INTEGER NOT NULL, suite_id TEXT,
  launch_key TEXT NOT NULL CHECK(length(launch_key) BETWEEN 1 AND 200),
  status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','interrupted')),
  config_snapshot_json TEXT NOT NULL CHECK(json_valid(config_snapshot_json) AND length(config_snapshot_json)<=1048576
    AND COALESCE(json_type(config_snapshot_json,'$.repetitions')='integer',0)
    AND json_extract(config_snapshot_json,'$.repetitions') BETWEEN 1 AND 100),
  config_hash TEXT NOT NULL CHECK(length(config_hash)=64),
  hardware_snapshot_json TEXT NOT NULL CHECK(json_valid(hardware_snapshot_json) AND length(hardware_snapshot_json)<=65536),
  app_version TEXT NOT NULL, source_revision TEXT NOT NULL, evaluator_version TEXT NOT NULL,
  created_at TEXT NOT NULL, started_at TEXT, finished_at TEXT,
  UNIQUE(workspace_id,id), UNIQUE(workspace_id,launch_key),
  UNIQUE(workspace_id,id,dataset_id,dataset_version),
  FOREIGN KEY(dataset_id,dataset_version) REFERENCES benchmark_datasets(id,version),
  FOREIGN KEY(workspace_id,suite_id) REFERENCES benchmark_suites(workspace_id,id)
);
CREATE UNIQUE INDEX one_active_benchmark ON benchmark_runs((1)) WHERE status='running';
CREATE INDEX benchmark_runs_workspace ON benchmark_runs(workspace_id,created_at,id);
CREATE TABLE benchmark_run_models (
  run_id TEXT NOT NULL, model_slot INTEGER NOT NULL CHECK(model_slot >= 0), workspace_id TEXT NOT NULL,
  profile_snapshot_json TEXT NOT NULL CHECK(json_valid(profile_snapshot_json) AND length(profile_snapshot_json)<=1048576),
  profile_hash TEXT NOT NULL CHECK(length(profile_hash)=64),
  PRIMARY KEY(run_id,model_slot), UNIQUE(workspace_id,run_id,model_slot),
  FOREIGN KEY(workspace_id,run_id) REFERENCES benchmark_runs(workspace_id,id)
);
CREATE TABLE benchmark_case_runs (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, run_id TEXT NOT NULL, model_slot INTEGER NOT NULL,
  dataset_id TEXT NOT NULL, dataset_version INTEGER NOT NULL, case_id TEXT NOT NULL,
  repetition INTEGER NOT NULL CHECK(repetition > 0), arm TEXT NOT NULL CHECK(arm='single'),
  status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','interrupted','unsupported','blocked')),
  UNIQUE(workspace_id,id), UNIQUE(run_id,model_slot,case_id,repetition,arm),
  FOREIGN KEY(workspace_id,run_id,dataset_id,dataset_version)
    REFERENCES benchmark_runs(workspace_id,id,dataset_id,dataset_version),
  FOREIGN KEY(workspace_id,run_id,model_slot) REFERENCES benchmark_run_models(workspace_id,run_id,model_slot),
  FOREIGN KEY(dataset_id,dataset_version,case_id) REFERENCES benchmark_cases(dataset_id,dataset_version,case_id)
);
CREATE INDEX benchmark_case_runs_status ON benchmark_case_runs(run_id,status,model_slot,case_id,repetition);
CREATE TRIGGER benchmark_repetition_limit BEFORE INSERT ON benchmark_case_runs
WHEN NEW.repetition > COALESCE((SELECT json_extract(config_snapshot_json,'$.repetitions') FROM benchmark_runs WHERE id=NEW.run_id),0)
BEGIN SELECT RAISE(ABORT,'Benchmark repetition is outside the frozen plan'); END;
CREATE TABLE benchmark_attempts (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, case_run_id TEXT NOT NULL,
  attempt_no INTEGER NOT NULL CHECK(attempt_no > 0), execution_run_id TEXT UNIQUE,
  status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','interrupted','unsupported','blocked')),
  request_disposition TEXT NOT NULL CHECK(request_disposition IN ('not_sent','unknown','finished')),
  started_at TEXT, finished_at TEXT, error_kind TEXT,
  UNIQUE(workspace_id,id), UNIQUE(case_run_id,attempt_no),
  FOREIGN KEY(workspace_id,case_run_id) REFERENCES benchmark_case_runs(workspace_id,id),
  FOREIGN KEY(workspace_id,execution_run_id) REFERENCES execution_runs(workspace_id,id)
);
CREATE UNIQUE INDEX one_active_benchmark_attempt ON benchmark_attempts(case_run_id) WHERE status='running';
CREATE TABLE benchmark_evaluations (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, attempt_id TEXT NOT NULL,
  evaluator_id TEXT NOT NULL, evaluator_version TEXT NOT NULL,
  scoring_config_hash TEXT NOT NULL CHECK(length(scoring_config_hash)=64),
  output_hash TEXT NOT NULL CHECK(length(output_hash)=64),
  metrics_json TEXT NOT NULL CHECK(json_valid(metrics_json) AND length(metrics_json)<=1048576),
  details_json TEXT NOT NULL CHECK(json_valid(details_json) AND length(details_json)<=1048576),
  status TEXT NOT NULL CHECK(status IN ('completed','failed','needs_review')), created_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id,attempt_id) REFERENCES benchmark_attempts(workspace_id,id)
);
CREATE INDEX benchmark_evaluations_attempt ON benchmark_evaluations(attempt_id,created_at,id);
CREATE TABLE benchmark_resource_samples (
  attempt_id TEXT NOT NULL, seq INTEGER NOT NULL CHECK(seq > 0), workspace_id TEXT NOT NULL,
  t_rel_ms INTEGER NOT NULL CHECK(t_rel_ms >= 0), source TEXT NOT NULL, scope TEXT NOT NULL, device TEXT,
  values_json TEXT NOT NULL CHECK(json_valid(values_json) AND length(values_json)<=65536),
  availability TEXT NOT NULL CHECK(availability IN ('available','partial','unavailable')),
  sample_interval_ms INTEGER NOT NULL CHECK(sample_interval_ms > 0),
  PRIMARY KEY(attempt_id,seq),
  FOREIGN KEY(workspace_id,attempt_id) REFERENCES benchmark_attempts(workspace_id,id)
);
CREATE INDEX benchmark_samples_time ON benchmark_resource_samples(attempt_id,t_rel_ms);
CREATE TABLE benchmark_events (
  run_id TEXT NOT NULL, seq INTEGER NOT NULL CHECK(seq > 0), workspace_id TEXT NOT NULL,
  kind TEXT NOT NULL, occurred_at TEXT NOT NULL, attempt_id TEXT,
  payload_json TEXT NOT NULL CHECK(json_valid(payload_json) AND length(payload_json)<=1048576),
  PRIMARY KEY(run_id,seq),
  FOREIGN KEY(workspace_id,run_id) REFERENCES benchmark_runs(workspace_id,id),
  FOREIGN KEY(workspace_id,attempt_id) REFERENCES benchmark_attempts(workspace_id,id)
);
CREATE TRIGGER benchmark_event_attempt_scope BEFORE INSERT ON benchmark_events
WHEN NEW.attempt_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM benchmark_attempts a JOIN benchmark_case_runs c ON c.id=a.case_run_id
  WHERE a.id=NEW.attempt_id AND c.run_id=NEW.run_id AND a.workspace_id=NEW.workspace_id
)
BEGIN SELECT RAISE(ABORT,'Benchmark event belongs to another experiment'); END;

CREATE TRIGGER benchmark_execution_scope_insert BEFORE INSERT ON benchmark_attempts
WHEN NEW.execution_run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id
  WHERE r.id=NEW.execution_run_id AND r.workspace_id=NEW.workspace_id AND c.purpose='benchmark'
)
BEGIN SELECT RAISE(ABORT,'Benchmark attempts require benchmark execution'); END;
CREATE TRIGGER benchmark_execution_scope_update BEFORE UPDATE OF execution_run_id ON benchmark_attempts
WHEN NEW.execution_run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id
  WHERE r.id=NEW.execution_run_id AND r.workspace_id=NEW.workspace_id AND c.purpose='benchmark'
)
BEGIN SELECT RAISE(ABORT,'Benchmark attempts require benchmark execution'); END;
CREATE TRIGGER immutable_benchmark_execution_binding BEFORE UPDATE OF execution_run_id ON benchmark_attempts
WHEN OLD.execution_run_id IS NOT NULL AND NEW.execution_run_id IS NOT OLD.execution_run_id
BEGIN SELECT RAISE(ABORT,'Benchmark execution binding is immutable'); END;

CREATE TRIGGER benchmark_blocks_knowledge_origin BEFORE INSERT ON knowledge_nodes
WHEN NEW.source_message_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM messages m JOIN conversations c ON c.id=m.conversation_id AND c.workspace_id=m.workspace_id
  WHERE m.id=NEW.source_message_id AND c.purpose='benchmark'
)
BEGIN SELECT RAISE(ABORT,'Benchmark output cannot become production knowledge'); END;
CREATE TRIGGER benchmark_blocks_context_knowledge BEFORE INSERT ON context_knowledge_items
WHEN EXISTS (SELECT 1 FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id
  WHERE r.id=NEW.run_id AND c.purpose='benchmark')
BEGIN SELECT RAISE(ABORT,'Local benchmark baseline cannot select production knowledge'); END;
CREATE TRIGGER benchmark_blocks_retrieval_results BEFORE INSERT ON retrieval_results
WHEN EXISTS (SELECT 1 FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id
  WHERE r.id=NEW.run_id AND c.purpose='benchmark')
BEGIN SELECT RAISE(ABORT,'Local benchmark baseline cannot use production retrieval'); END;
CREATE TRIGGER benchmark_blocks_retrieval_setting_insert BEFORE INSERT ON conversation_retrieval_settings
WHEN EXISTS (SELECT 1 FROM conversations WHERE id=NEW.conversation_id AND purpose='benchmark')
BEGIN SELECT RAISE(ABORT,'Benchmark conversations cannot change chat retrieval settings'); END;
CREATE TRIGGER benchmark_blocks_retrieval_setting_update BEFORE UPDATE ON conversation_retrieval_settings
WHEN EXISTS (SELECT 1 FROM conversations WHERE id=NEW.conversation_id AND purpose='benchmark')
BEGIN SELECT RAISE(ABORT,'Benchmark conversations cannot change chat retrieval settings'); END;

CREATE TRIGGER immutable_benchmark_dataset_update BEFORE UPDATE ON benchmark_datasets
BEGIN SELECT RAISE(ABORT,'Dataset versions are immutable'); END;
CREATE TRIGGER immutable_benchmark_dataset_delete BEFORE DELETE ON benchmark_datasets
BEGIN SELECT RAISE(ABORT,'Dataset versions are immutable'); END;
CREATE TRIGGER immutable_benchmark_case_update BEFORE UPDATE ON benchmark_cases
BEGIN SELECT RAISE(ABORT,'Dataset cases are immutable'); END;
CREATE TRIGGER immutable_benchmark_case_delete BEFORE DELETE ON benchmark_cases
BEGIN SELECT RAISE(ABORT,'Dataset cases are immutable'); END;
CREATE TRIGGER immutable_benchmark_suite_scope BEFORE UPDATE OF id,workspace_id ON benchmark_suites
BEGIN SELECT RAISE(ABORT,'Suite identity is immutable'); END;
CREATE TRIGGER immutable_benchmark_run_config BEFORE UPDATE OF id,workspace_id,dataset_id,dataset_version,suite_id,launch_key,config_snapshot_json,config_hash,hardware_snapshot_json,app_version,source_revision,evaluator_version,created_at ON benchmark_runs
BEGIN SELECT RAISE(ABORT,'Experiment configuration is immutable'); END;
CREATE TRIGGER immutable_benchmark_run_delete BEFORE DELETE ON benchmark_runs
BEGIN SELECT RAISE(ABORT,'Experiment history is immutable'); END;
CREATE TRIGGER immutable_benchmark_model_update BEFORE UPDATE ON benchmark_run_models
BEGIN SELECT RAISE(ABORT,'Model snapshots are immutable'); END;
CREATE TRIGGER immutable_benchmark_model_delete BEFORE DELETE ON benchmark_run_models
BEGIN SELECT RAISE(ABORT,'Model snapshots are immutable'); END;
CREATE TRIGGER immutable_benchmark_case_run_identity BEFORE UPDATE OF id,workspace_id,run_id,model_slot,dataset_id,dataset_version,case_id,repetition,arm ON benchmark_case_runs
BEGIN SELECT RAISE(ABORT,'Case plan is immutable'); END;
CREATE TRIGGER immutable_benchmark_case_run_delete BEFORE DELETE ON benchmark_case_runs
BEGIN SELECT RAISE(ABORT,'Case plan is immutable'); END;
CREATE TRIGGER immutable_benchmark_attempt_identity BEFORE UPDATE OF id,workspace_id,case_run_id,attempt_no ON benchmark_attempts
BEGIN SELECT RAISE(ABORT,'Attempt identity is immutable'); END;
CREATE TRIGGER immutable_benchmark_attempt_terminal BEFORE UPDATE ON benchmark_attempts
WHEN OLD.status IN ('completed','failed','cancelled','interrupted','unsupported','blocked')
BEGIN SELECT RAISE(ABORT,'Finished attempts are immutable'); END;
CREATE TRIGGER immutable_benchmark_attempt_delete BEFORE DELETE ON benchmark_attempts
BEGIN SELECT RAISE(ABORT,'Attempt history is immutable'); END;
CREATE TRIGGER immutable_benchmark_evaluation_update BEFORE UPDATE ON benchmark_evaluations
BEGIN SELECT RAISE(ABORT,'Evaluations are immutable'); END;
CREATE TRIGGER immutable_benchmark_evaluation_delete BEFORE DELETE ON benchmark_evaluations
BEGIN SELECT RAISE(ABORT,'Evaluations are immutable'); END;
CREATE TRIGGER immutable_benchmark_sample_update BEFORE UPDATE ON benchmark_resource_samples
BEGIN SELECT RAISE(ABORT,'Resource samples are immutable'); END;
CREATE TRIGGER immutable_benchmark_sample_delete BEFORE DELETE ON benchmark_resource_samples
BEGIN SELECT RAISE(ABORT,'Resource samples are immutable'); END;
CREATE TRIGGER immutable_benchmark_event_update BEFORE UPDATE ON benchmark_events
BEGIN SELECT RAISE(ABORT,'Experiment events are immutable'); END;
CREATE TRIGGER immutable_benchmark_event_delete BEFORE DELETE ON benchmark_events
BEGIN SELECT RAISE(ABORT,'Experiment events are immutable'); END;

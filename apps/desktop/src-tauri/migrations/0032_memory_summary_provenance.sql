ALTER TABLE conversation_summaries
  ADD COLUMN source_run_id TEXT REFERENCES execution_runs(id) ON DELETE SET NULL;

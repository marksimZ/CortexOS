CREATE TABLE rag_evaluations (
 workspace_id TEXT NOT NULL REFERENCES workspaces(id),
 id TEXT NOT NULL, created_at TEXT NOT NULL,
 body_json TEXT NOT NULL CHECK(json_valid(body_json)), body_hash TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id)
);
CREATE INDEX rag_evaluations_history ON rag_evaluations(workspace_id,created_at DESC,id DESC);
CREATE TRIGGER immutable_rag_evaluation BEFORE UPDATE ON rag_evaluations
BEGIN SELECT RAISE(ABORT,'RAG evaluation reports are immutable'); END;
CREATE TRIGGER retain_rag_evaluation BEFORE DELETE ON rag_evaluations
BEGIN SELECT RAISE(ABORT,'Retain RAG evaluation evidence'); END;

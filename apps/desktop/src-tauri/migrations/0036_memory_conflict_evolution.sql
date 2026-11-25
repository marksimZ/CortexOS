ALTER TABLE memory_candidates ADD COLUMN replaces_node_id TEXT REFERENCES knowledge_nodes(id) ON DELETE SET NULL;
ALTER TABLE memory_candidates ADD COLUMN replaces_value TEXT;
ALTER TABLE memory_candidates ADD COLUMN conflict_reason TEXT;

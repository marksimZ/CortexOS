CREATE TABLE knowledge_validity (
 node_id TEXT NOT NULL, version INTEGER NOT NULL,
 valid_from TEXT, valid_to TEXT,
 PRIMARY KEY(node_id,version),
 FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version),
 CHECK(valid_from IS NOT NULL OR valid_to IS NOT NULL),
 CHECK(valid_from IS NULL OR valid_to IS NULL OR valid_from<valid_to)
);
CREATE TRIGGER knowledge_validity_update BEFORE UPDATE ON knowledge_validity
BEGIN SELECT RAISE(ABORT,'Knowledge validity is immutable'); END;
CREATE TRIGGER knowledge_validity_delete BEFORE DELETE ON knowledge_validity
BEGIN SELECT RAISE(ABORT,'Knowledge validity is immutable'); END;

CREATE TABLE knowledge_snapshots (
 workspace_id TEXT NOT NULL REFERENCES workspaces(id), id TEXT NOT NULL,
 name TEXT NOT NULL, created_at TEXT NOT NULL, fact_date TEXT NOT NULL,
 item_count INTEGER NOT NULL CHECK(item_count>0),
 PRIMARY KEY(workspace_id,id)
);
CREATE TABLE knowledge_snapshot_items (
 workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, node_id TEXT NOT NULL, version INTEGER NOT NULL,
 ordinal INTEGER NOT NULL CHECK(ordinal>0),
 PRIMARY KEY(workspace_id,snapshot_id,node_id),
 UNIQUE(workspace_id,snapshot_id,ordinal),
 FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,id),
 FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
 FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
CREATE TRIGGER knowledge_snapshot_update BEFORE UPDATE ON knowledge_snapshots
BEGIN SELECT RAISE(ABORT,'Knowledge snapshots are immutable'); END;
CREATE TRIGGER knowledge_snapshot_delete BEFORE DELETE ON knowledge_snapshots
BEGIN SELECT RAISE(ABORT,'Knowledge snapshots are immutable'); END;
CREATE TRIGGER knowledge_snapshot_item_update BEFORE UPDATE ON knowledge_snapshot_items
BEGIN SELECT RAISE(ABORT,'Knowledge snapshot items are immutable'); END;
CREATE TRIGGER knowledge_snapshot_item_delete BEFORE DELETE ON knowledge_snapshot_items
BEGIN SELECT RAISE(ABORT,'Knowledge snapshot items are immutable'); END;
CREATE TRIGGER knowledge_snapshot_item_limit BEFORE INSERT ON knowledge_snapshot_items
WHEN NEW.ordinal > (SELECT item_count FROM knowledge_snapshots WHERE workspace_id=NEW.workspace_id AND id=NEW.snapshot_id)
BEGIN SELECT RAISE(ABORT,'Knowledge snapshot is already complete'); END;

CREATE VIEW rag_current_knowledge AS
SELECT n.workspace_id,n.id AS node_id,v.version
FROM knowledge_nodes n
JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
LEFT JOIN knowledge_validity d ON d.node_id=n.id AND d.version=v.version
WHERE v.kind IN ('knowledge','context','decision')
AND (d.valid_from IS NULL OR d.valid_from<=date('now','localtime'))
AND (d.valid_to IS NULL OR d.valid_to>date('now','localtime'));

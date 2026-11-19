CREATE TABLE knowledge_experimental_branches (
 workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, base_snapshot_id TEXT NOT NULL,
 overrides_json TEXT NOT NULL CHECK(json_valid(overrides_json)),
 PRIMARY KEY(workspace_id,snapshot_id),
 FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,id),
 FOREIGN KEY(workspace_id,base_snapshot_id) REFERENCES knowledge_snapshots(workspace_id,id),
 CHECK(snapshot_id<>base_snapshot_id)
);
CREATE TRIGGER knowledge_experimental_branch_update BEFORE UPDATE ON knowledge_experimental_branches
BEGIN SELECT RAISE(ABORT,'Knowledge experiment is immutable'); END;
CREATE TRIGGER knowledge_experimental_branch_delete BEFORE DELETE ON knowledge_experimental_branches
BEGIN SELECT RAISE(ABORT,'Knowledge experiment is immutable'); END;

CREATE TABLE knowledge_snapshot_conflicts (
 workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, edge_id TEXT NOT NULL, edge_version INTEGER NOT NULL,
 PRIMARY KEY(workspace_id,snapshot_id,edge_id),
 FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,id),
 FOREIGN KEY(workspace_id,edge_id) REFERENCES knowledge_edges(workspace_id,id),
 FOREIGN KEY(edge_id,edge_version) REFERENCES knowledge_edge_versions(edge_id,version)
);
CREATE TRIGGER knowledge_snapshot_conflict_update BEFORE UPDATE ON knowledge_snapshot_conflicts
BEGIN SELECT RAISE(ABORT,'Knowledge snapshot relationships are immutable'); END;
CREATE TRIGGER knowledge_snapshot_conflict_delete BEFORE DELETE ON knowledge_snapshot_conflicts
BEGIN SELECT RAISE(ABORT,'Knowledge snapshot relationships are immutable'); END;

INSERT INTO knowledge_snapshot_conflicts
SELECT s.workspace_id,s.id,v.edge_id,v.version
FROM knowledge_snapshots s JOIN knowledge_edges e ON e.workspace_id=s.workspace_id
JOIN knowledge_edge_versions v ON v.edge_id=e.id AND v.workspace_id=e.workspace_id
 AND v.version=(SELECT MAX(old.version) FROM knowledge_edge_versions old
                WHERE old.edge_id=e.id AND old.created_at<s.created_at)
JOIN knowledge_snapshot_items a ON a.workspace_id=s.workspace_id AND a.snapshot_id=s.id AND a.node_id=v.from_node_id
JOIN knowledge_snapshot_items b ON b.workspace_id=s.workspace_id AND b.snapshot_id=s.id AND b.node_id=v.to_node_id
WHERE v.active=1 AND v.relation='contradicts';

CREATE VIEW knowledge_version_lineage AS
SELECT node_id,version,
 LAG(version) OVER (PARTITION BY node_id ORDER BY version) AS replaces_version,
 LEAD(version) OVER (PARTITION BY node_id ORDER BY version) AS replaced_by_version
FROM knowledge_approval_events;

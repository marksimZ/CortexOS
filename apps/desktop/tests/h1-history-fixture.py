import sqlite3
import sys
from pathlib import Path

# Playwright supplies its own freshly created test data directory.
path = Path(sys.argv[1]).resolve(strict=True)
workspace = sys.argv[2]
when = '2026-11-20T00:00:00Z'
with sqlite3.connect(path) as connection:
    connection.execute('PRAGMA foreign_keys = ON')
    name = connection.execute('SELECT name FROM workspaces WHERE id=?', (workspace,)).fetchone()
    if name != ('H1 合成长历史',):
        raise RuntimeError('Expected the dedicated Playwright fixture workspace')
    for count in (100, 500, 1000):
        conversation = f'h1-fixture-{count}'
        connection.execute('INSERT INTO conversations (id,workspace_id,title,created_at,updated_at) VALUES (?,?,?,?,?)', (conversation, workspace, f'{count} 条合成消息', when, when))
        for index in range(count // 2):
            run = f'{conversation}-run-{index}'
            ids = (f'{run}-question', f'{run}-answer')
            connection.execute('INSERT INTO execution_runs (id,workspace_id,conversation_id,assistant_message_id,model_id,model_config_json,context_manifest_json,status,started_at,finished_at) VALUES (?,?,?,?,?,?,?,?,?,?)', (run, workspace, conversation, ids[1], 'fixture', '{}', '{}', 'completed', when, when))
            connection.executemany('INSERT INTO messages (id,workspace_id,conversation_id,seq,role,content,status,model_run_id,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?)', (
                (message, workspace, conversation, index*2+offset+1, role, f'第 {index*2+offset+1} 条 **合成**消息。\n\n- 补充一行内容', 'completed', run, when, when)
                for offset, (role,message) in enumerate(zip(('user','assistant'), ids))
            ))

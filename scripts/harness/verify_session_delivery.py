"""Read-only audit of this slice's isolated QA databases and owned containers."""
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
FIRST = ROOT / 'artifacts/harness-codex-session-real/harness-codex-session-one--469b9-ion-keeps-the-actual-result-harness-codex-session'
LAST = ROOT / 'artifacts/harness-codex-session-real-text2/harness-codex-session-one--469b9-ion-keeps-the-actual-result-harness-codex-session'
CONTROL = ROOT / 'artifacts/harness-codex-session-controlled-final/harness-codex-session-cont-eb341-and-restart-without-a-model-harness-codex-session'

def connect(path):
    return sqlite3.connect(path.as_uri() + '?mode=ro', uri=True)

def snapshot(connection, job):
    return (connection.execute('SELECT * FROM benchmark_generation_jobs WHERE id=?', (job,)).fetchall(),
            connection.execute('SELECT * FROM benchmark_generated_cases WHERE job_id=? ORDER BY rowid', (job,)).fetchall())

def inspect(folder):
    with connect(folder / 'data/cortexos.sqlite3') as c:
        assert c.execute('PRAGMA integrity_check').fetchall() == [('ok',)]
        assert not c.execute('PRAGMA foreign_key_check').fetchall()
        migrations = c.execute('SELECT version,checksum FROM schema_migrations ORDER BY version').fetchall()
        assert migrations[-1][0] == 11
        records = []
        for job, raw, digest, status in c.execute("SELECT id,plan_json,plan_hash,status FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose')='harness_codex_session'"):
            assert hashlib.sha256(raw.encode()).hexdigest() == digest
            events = []
            for event, proof in c.execute('SELECT candidate_json,validation_json FROM benchmark_generated_cases WHERE job_id=?', (job,)):
                assert json.loads(proof)['contentHash'] == hashlib.sha256(event.encode()).hexdigest()
                events.append(json.loads(event))
            plan = json.loads(raw)
            assert any(e['kind'] == 'cleanup' and e['data']['clean'] and e['data']['name'] == plan['containerName'] for e in events)
            reply = subprocess.run(['docker', '--host', plan['environment']['endpoint'], 'container', 'ls', '-a', '--filter', 'label=com.cortexos.validation=' + job, '--format', '{{.Names}}'], capture_output=True, timeout=5, check=True)
            assert not reply.stdout.strip(), (job, reply.stdout)
            assert not (Path(tempfile.gettempdir()) / ('cortexos-codex-seccomp-' + job + '.json')).exists()
            records.append({'id':job,'status':status,'clean':True,'forwardingAttempts':sum(e['kind']=='model_requested' for e in events)})
        counts = {table:c.execute('SELECT count(*) FROM ' + table).fetchone()[0] for table in ['messages','knowledge_nodes','knowledge_versions','execution_runs','context_snapshots','benchmark_runs']}
        return {'schema':migrations,'integrity':'ok','foreignKeys':[],'ordinaryCounts':counts,'records':records}

first = inspect(FIRST)
last = inspect(LAST)
control = inspect(CONTROL)
assert first['schema'] == last['schema'] == control['schema']
assert first['ordinaryCounts'] == last['ordinaryCounts'] == control['ordinaryCounts']
legacy = json.loads((FIRST / 'actual-session.json').read_text(encoding='utf-8'))['actual']
with connect(FIRST / 'data/cortexos.sqlite3') as a, connect(LAST / 'data/cortexos.sqlite3') as b, connect(CONTROL / 'data/cortexos.sqlite3') as c:
    assert snapshot(a, legacy['id']) == snapshot(b, legacy['id']) == snapshot(c, legacy['id'])
# GET only: this never loads, prewarms, unloads or queries an answer from a model.
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://127.0.0.1:11434/api/ps', timeout=5) as response:
    loaded = json.load(response)['models']
assert loaded == []
result = {'schemaVersion':11,'first':first,'final':last,'controlled':control,'legacyBytesUnchanged':True,'ordinaryDataUnchanged':True,'loadedModels':loaded,'additionalInferenceRequests':0}
(LAST / 'database-verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps({'integrity':'ok','schemaVersion':11,'ordinaryCounts':last['ordinaryCounts'],'legacyBytesUnchanged':True,'sessionRecordsChecked':len(first['records'])+len(last['records'])+len(control['records']),'ownedContainersAbsent':True,'loadedModels':loaded}))

"""Run only the three explicitly approved, fixed offline compatibility groups."""
import hashlib
import json
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts/harness-codex-namespace'
ENDPOINT = 'npipe:////./pipe/dockerDesktopLinuxEngine'
review = json.loads((OUT / 'review.json').read_text())
manifest = json.loads((ROOT / 'artifacts/harness-codex-runtime/runtime.json').read_text())
image = manifest['imageId']
owner = str(uuid.uuid4())
results = []
bootstrap = '''import json,os,subprocess,sys
p=json.load(sys.stdin)
os.makedirs(os.environ['CODEX_HOME'],mode=0o700,exist_ok=True)
observed={}
for key in ('namespace','codex'):
 r=subprocess.run(['python3','-I','-S','-u','-c',p[key]],capture_output=True,timeout=32)
 observed[key]={'exitCode':r.returncode,'stdout':r.stdout.decode(),'stderr':r.stderr.decode()}
print(json.dumps(observed))
'''
payload = json.dumps({'namespace': (ROOT / 'scripts/harness/namespace_probe.py').read_text(),
                      'codex': (ROOT / 'apps/desktop/src-tauri/src/benchmark/python/codex_probe.py').read_text()}).encode()

def command(args, data=None, timeout=10):
    reply = subprocess.run(['docker', '--host', ENDPOINT, *args], input=data, capture_output=True, timeout=timeout)
    if len(reply.stdout) + len(reply.stderr) > 65536:
        raise RuntimeError('output_limit')
    return reply

def save():
    (OUT / 'comparison.json').write_text(json.dumps({'owner':owner,'endpoint':ENDPOINT,'imageId':image,'groups':results,'modelCalls':0},indent=2),encoding='utf-8')

for variant in ('baseline', 'userns', 'bubblewrap'):
    path = OUT / ('moby-default.json' if variant == 'baseline' else variant + '.json')
    expected = review['source']['ProfileSha256'] if variant == 'baseline' else next(v['sha256'] for v in review['variants'] if v['name'] == variant)
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit('Profile fingerprint mismatch; no container started.')
    name = 'cortexos-bench-' + str(uuid.uuid4())
    record = {'variant':variant,'name':name,'owner':owner,'profileHash':expected,'clean':False}
    results.append(record);save()  # Record ownership before create.
    try:
        create = command(['create','--pull=never','--name',name,'--network=none','--read-only','--cpus=1',
                          '--memory=268435456','--memory-swap=268435456','--pids-limit=64','--cap-drop=ALL',
                          '--security-opt=no-new-privileges','--security-opt=seccomp='+str(path),
                          '--user=65534:65534','--label=com.cortexos.validation='+owner,
                          '--label=com.cortexos.purpose=harness_codex_compatibility',
                          '--tmpfs=/tmp:rw,noexec,nosuid,size=16m,mode=1777',
                          '--tmpfs=/workspace:rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534',
                          '--tmpfs=/home/cortexos:rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534',
                          '--shm-size=8m','--ulimit=nofile=64:64','--ulimit=core=0:0','--ulimit=fsize=1048576:1048576',
                          '--log-driver=none','--no-healthcheck','--workdir=/workspace','--entrypoint=python3','-i',image,
                          '-I','-S','-u','-c',bootstrap])
        if create.returncode:
            raise RuntimeError('create_failed: '+create.stderr.decode(errors='replace'))
        inspected = command(['inspect',name,'--format','{{json .}}'])
        config = json.loads(inspected.stdout)
        record['configuration'] = config
        if config['Config']['Labels']['com.cortexos.validation'] != owner or config['Config']['Image'] != image:
            raise RuntimeError('owner_mismatch')
        reply = command(['start','--attach','--interactive',name],payload,45)
        record['exitCode'] = reply.returncode
        record['stderr'] = reply.stderr.decode(errors='replace')
        record['observation'] = json.loads(reply.stdout) if reply.returncode == 0 else {'output':reply.stdout.decode(errors='replace')}
    except Exception as problem:
        record['error'] = str(problem)
    finally:
        found=command(['container','ls','-a','--filter','name=^/'+name+'$','--format','{{.ID}}'])
        if found.returncode == 0 and not found.stdout.strip():
            record['clean'] = True
        else:
            inspected=command(['inspect',name,'--format','{{json .}}'])
            if inspected.returncode == 0:
                config=json.loads(inspected.stdout)
                if config['Config']['Labels'].get('com.cortexos.validation') == owner and config['Config']['Image'] == image:
                    removed=command(['rm','--force','--volumes',name])
                    record['clean'] = removed.returncode == 0
        save()
    print(json.dumps({'variant':variant,'error':record.get('error'),'clean':record['clean'],'observation':record.get('observation')},ensure_ascii=False))
    if not record['clean']:
        raise SystemExit('Owned container cleanup failed; stop further groups.')

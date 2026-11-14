"""Offline protocol observation. The responder never contacts a model service."""
import json
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts/harness-codex-transport'
OUT.mkdir(parents=True, exist_ok=True)
RUNTIME = json.loads((ROOT / 'artifacts/harness-codex-runtime/runtime.json').read_text())
PROFILE = ROOT / 'apps/desktop/src-tauri/catalogs/codex-seccomp/profile-v1.json'
CODE = r'''
import http.server,json,os,subprocess,threading
from pathlib import Path
requests=[]
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def do_POST(self):
        size=int(self.headers.get('Content-Length','0'))
        if size>131072:raise ValueError('request_limit')
        requests.append({'path':self.path,'body':json.loads(self.rfile.read(size))})
        self.send_response(400);self.send_header('Content-Type','application/json');self.end_headers()
        self.wfile.write(b'{"error":{"message":"offline_protocol_probe_no_model","type":"invalid_request_error"}}')
os.makedirs('/home/cortexos/.codex',exist_ok=True)
Path('/home/cortexos/instructions.md').write_text('Solve the task using the provided workspace and tools. Do not request network or elevated permissions.')
server=http.server.HTTPServer(('127.0.0.1',0),Server)
threading.Thread(target=server.serve_forever,daemon=True).start()
options={'model_provider':'cortexos_local','model_providers.cortexos_local.name':'CortexOS local relay','model_providers.cortexos_local.base_url':f'http://127.0.0.1:{server.server_port}/v1','model_providers.cortexos_local.wire_api':'responses','model_providers.cortexos_local.request_max_retries':0,'model_providers.cortexos_local.stream_max_retries':0,'model_instructions_file':'/home/cortexos/instructions.md','model_context_window':4096,'sandbox_workspace_write.exclude_slash_tmp':True,'sandbox_workspace_write.exclude_tmpdir_env_var':True,'sandbox_workspace_write.network_access':False,'features.multi_agent':False,'features.goals':False,'web_search':'disabled','features.enable_request_compression':False}
args=['codex','--no-daemon','-a','on-request']
for key,value in options.items():args.extend(['-c',key+'='+json.dumps(value)])
args+=['exec','--ignore-user-config','--ignore-rules','--ephemeral','--skip-git-repo-check','--json','--color','never','--sandbox','workspace-write','--model','cortexos-probe','Inspect the workspace task. This is an offline protocol probe.']
try:
    result=subprocess.run(args,capture_output=True,timeout=25)
    files=[{'path':str(p),'size':p.stat().st_size} for p in Path('/home/cortexos').rglob('*') if p.is_file()]
    print(json.dumps({'modelCalls':0,'requests':requests,'exitCode':result.returncode,'stdout':result.stdout.decode(errors='replace')[:24000],'stderr':result.stderr.decode(errors='replace')[:8000],'runtimeFiles':files}))
finally:server.shutdown()
'''
name = 'cortexos-transport-probe-' + str(uuid.uuid4())
args = ['docker', 'create', '--pull=never', '--name', name, '--network=none', '--read-only',
        '--user=65534:65534', '--cpus=1', '--memory=268435456', '--memory-swap=268435456',
        '--pids-limit=64', '--cap-drop=ALL', '--security-opt=no-new-privileges',
        '--ulimit=nofile=64:64', '--ulimit=core=0:0', '--ulimit=fsize=4194304:4194304',
        '--security-opt=seccomp=' + str(PROFILE), '--log-driver=none',
        '--tmpfs=/workspace:rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534',
        '--tmpfs=/tmp:rw,noexec,nosuid,nodev,size=16m,mode=1777',
        '--tmpfs=/home/cortexos:rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534',
        '--workdir=/workspace', '--entrypoint=python3', RUNTIME['imageId'], '-I', '-S', '-u', '-c', CODE]
try:
    subprocess.run(args, check=True, capture_output=True, timeout=10)
    result = subprocess.run(['docker', 'start', '-a', name], capture_output=True, timeout=40)
    (OUT / 'protocol.json').write_bytes(result.stdout)
    (OUT / 'stderr.txt').write_bytes(result.stderr)
    print(json.dumps({'exitCode':result.returncode,'observation':str(OUT/'protocol.json'),'modelCalls':0}))
finally:
    subprocess.run(['docker', 'rm', '-f', name], capture_output=True, timeout=10, check=True)

"""Offline protocol observation. The responder never contacts a model service."""
import json
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts/harness-aider-transport'
OUT.mkdir(parents=True, exist_ok=True)
DOCKER=['docker','--host','npipe:////./pipe/dockerDesktopLinuxEngine']
RUNTIME = json.loads((ROOT / 'artifacts/harness-aider-runtime/runtime.json').read_text())
if RUNTIME['imageId']!='sha256:82b98ea28b075414873b52a67228fd9c0cb2b25ee564db0a3d7d673699f6cc3e': raise RuntimeError('Unreviewed Aider image')
CODE = r'''
import http.server,json,os,subprocess,threading
from pathlib import Path
requests=[]
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def do_POST(self):
        size=int(self.headers.get('Content-Length','0'))
        if self.path!='/v1/chat/completions' or size<1 or size>65536 or len(requests)>=4 or self.headers.get('Transfer-Encoding'):
            self.send_error(400);return
        requests.append({'path':self.path,'body':json.loads(self.rfile.read(size))})
        self.send_response(200);self.send_header('Content-Type','application/json');self.end_headers()
        reply={'id':'offline-fixture','object':'chat.completion','created':0,'model':'cortexos-probe','choices':[{'index':0,'finish_reason':'stop','message':{'role':'assistant','content':'solution.py\n```python\ndef adjust(n):\n    return max(0, n)\n```'}}],'usage':{'prompt_tokens':0,'completion_tokens':0,'total_tokens':0}}
        self.wfile.write(json.dumps(reply).encode())
Path('/home/cortexos/.aider/caches').mkdir(parents=True)
Path('/home/cortexos/.aider/caches/model_prices_and_context_window.json').write_text('{"offline_probe_marker": {}}')
Path('/home/cortexos/empty.yml').write_text('{}\n')
Path('/workspace/solution.py').write_text('def adjust(n):\n    return n\n')
server=http.server.HTTPServer(('127.0.0.1',0),Server)
threading.Thread(target=server.serve_forever,daemon=True).start()
os.environ.clear()
os.environ.update({'PATH':'/usr/local/bin:/usr/bin:/bin','HOME':'/home/cortexos','TMPDIR':'/tmp','OPENAI_API_KEY':'offline-fixture-not-a-credential','OPENAI_API_BASE':f'http://127.0.0.1:{server.server_port}/v1','LITELLM_LOCAL_MODEL_COST_MAP':'True','AIDER_ANALYTICS':'false','AIDER_CHECK_UPDATE':'false','DO_NOT_TRACK':'1','GIT_CONFIG_NOSYSTEM':'1'})
args=['/usr/local/bin/python','-I','-m','aider','--model','openai/cortexos-probe','--no-stream','--no-git','--no-auto-commits','--no-auto-lint','--no-auto-test','--no-analytics','--no-check-update','--no-show-release-notes','--no-restore-chat-history','--no-suggest-shell-commands','--no-detect-urls','--no-check-model-accepts-settings','--no-show-model-warnings','--map-tokens','0','--edit-format','whole','--config','/home/cortexos/empty.yml','--env-file','/dev/null','--message','Change adjust to return 0 for negative n.','/workspace/solution.py']
try:
    result=subprocess.run(args,capture_output=True,timeout=25)
    files=[{'path':str(p),'size':p.stat().st_size} for p in Path('/home/cortexos').rglob('*') if p.is_file()]
    print(json.dumps({'modelCalls':0,'fixtureOnly':True,'candidate':Path('/workspace/solution.py').read_text(),'requests':requests,'exitCode':result.returncode,'stdout':result.stdout.decode(errors='replace')[:24000],'stderr':result.stderr.decode(errors='replace')[:8000],'runtimeFiles':files}))
finally:server.shutdown()
'''
name = 'cortexos-aider-transport-' + str(uuid.uuid4())
args = DOCKER + ['create', '--pull=never', '--name', name, '--network=none', '--read-only',
        '--user=65534:65534', '--cpus=1', '--memory=268435456', '--memory-swap=268435456',
        '--pids-limit=64', '--cap-drop=ALL', '--security-opt=no-new-privileges',
        '--ulimit=nofile=64:64', '--ulimit=core=0:0', '--ulimit=fsize=4194304:4194304',
        '--log-driver=none',
        '--tmpfs=/workspace:rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534',
        '--tmpfs=/tmp:rw,noexec,nosuid,nodev,size=16m,mode=1777',
        '--tmpfs=/home/cortexos:rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534',
        '--workdir=/workspace', '--entrypoint=python3', RUNTIME['imageId'], '-I', '-S', '-u', '-c', CODE]
try:
    subprocess.run(args, check=True, capture_output=True, timeout=10)
    result = subprocess.run(DOCKER+['start', '-a', name], capture_output=True, timeout=40)
    output=OUT/('protocol-'+str(uuid.uuid4())+'.json'); output.write_bytes(result.stdout)
    (OUT / 'protocol.json').write_bytes(result.stdout)
    (OUT / 'stderr.txt').write_bytes(result.stderr)
    observed=json.loads(result.stdout)
    assert result.returncode==0 and observed['exitCode']==0 and len(observed['requests'])==1
    assert observed['requests'][0]['path']=='/v1/chat/completions'
    assert observed['candidate']=='def adjust(n):\n    return max(0, n)\n'
    assert 'raw.githubusercontent.com' not in observed['stdout']
    print(json.dumps({'exitCode':result.returncode,'observation':str(output),'modelCalls':0,'fixedReplyEditVerified':True}))
finally:
    subprocess.run(DOCKER+['rm', '-f', name], capture_output=True, timeout=10, check=True)

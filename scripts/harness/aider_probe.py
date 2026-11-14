"""Trusted offline CLI/outer-isolation probe. No task, credential or model endpoint."""
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import resource
import socket
import subprocess
import sys
import tempfile

def run(args):
    env={'PATH':'/usr/local/bin:/usr/bin:/bin','HOME':'/home/cortexos','TMPDIR':'/tmp','PYTHONDONTWRITEBYTECODE':'1','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','AIDER_ANALYTICS':'false','AIDER_CHECK_UPDATE':'false','AIDER_SHOW_RELEASE_NOTES':'false','LITELLM_LOCAL_MODEL_COST_MAP':'True','DO_NOT_TRACK':'1','TOKENIZERS_PARALLELISM':'false'}
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        def limits(): resource.setrlimit(resource.RLIMIT_FSIZE,(65536,65536))
        child=subprocess.Popen(['/usr/local/bin/python','-I','-m','aider',*args],cwd='/workspace',env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,preexec_fn=limits)
        timeout=False
        try:child.wait(timeout=8)
        except subprocess.TimeoutExpired:timeout=True;child.kill();child.wait()
        out.seek(0);err.seek(0)
        return {'arguments':args,'exitCode':child.returncode,'timeout':timeout,'stdout':out.read(65536).decode('utf-8',errors='replace'),'stderr':err.read(65536).decode('utf-8',errors='replace')}

def main():
    # Parent interpreter is started with -S; child CLI explicitly enables installed packages.
    sys.path.append('/usr/local/lib/python3.12/site-packages')
    dist=importlib.metadata.distribution('aider-chat')
    lock=Path('/opt/requirements.lock').read_bytes()
    status={}
    for line in Path('/proc/self/status').read_text().splitlines():
        key,_,value=line.partition(':')
        if key in ('CapEff','CapPrm','CapAmb','NoNewPrivs','Seccomp'):status[key]=value.strip()
    def denied(path):
        try:Path(path).write_text('probe');return False
        except OSError:return True
    sock=socket.socket();sock.settimeout(1)
    try:sock.connect(('192.0.2.1',80));network=False
    except OSError:network=True
    finally:sock.close()
    checks={'nonRoot':os.getuid()==65534,'noUsableCapabilities':all(int(status[k],16)==0 for k in ('CapEff','CapPrm','CapAmb')),'noNewPrivileges':status.get('NoNewPrivs')=='1','seccompActive':status.get('Seccomp')=='2','readonlyRoot':denied('/usr/local/cortexos-aider-probe'),'networkUnavailable':network,'noDockerSocket':not Path('/var/run/docker.sock').exists()}
    result={'protocol':1,'python':list(sys.version_info[:2]),'packageVersion':dist.version,'lockHash':hashlib.sha256(lock).hexdigest(),'outerChecks':checks,'version':run(['--version']),'help':run(['--help']),'modelCalls':0,'taskExecution':'Not Run','innerSandbox':'not_provided_by_this_probe'}
    print(json.dumps(result,ensure_ascii=False))

if __name__=='__main__':main()

"""The three explicitly approved offline Claude compatibility groups. No model."""
import hashlib,json,subprocess,uuid,sys
from pathlib import Path

repo=Path(__file__).resolve().parents[2]
out=repo/'artifacts/harness-claude-sandbox'
review=json.loads((out/'review.json').read_text())
runtime=json.loads((out/'runtime.json').read_text())
endpoint='npipe:////./pipe/dockerDesktopLinuxEngine'
owner=str(uuid.uuid4())
groups=[]
details=sys.argv[1:]==['--candidate-details']
boundary=sys.argv[1:]==['--candidate-boundary']
assert not sys.argv[1:] or details or boundary
payload=json.dumps({'namespace':(repo/'scripts/harness/namespace_probe.py').read_text(),'claude':(repo/'scripts/harness/claude_sandbox_probe.py').read_text()}).encode()
bootstrap='''import json,subprocess,sys
p=json.load(sys.stdin)
results={}
for key in ('namespace','claude'):
 r=subprocess.run(['python3','-I','-S','-u','-c',p[key].replace("if __name__=='__main__':", "if True:")],capture_output=True,timeout=22)
 results[key]={'exitCode':r.returncode,'stdout':r.stdout.decode(),'stderr':r.stderr.decode()}
print(json.dumps(results))
'''
def command(args,data=None,timeout=10):
    r=subprocess.run(['docker','--host',endpoint,*args],input=data,capture_output=True,timeout=timeout)
    if len(r.stdout)+len(r.stderr)>65536:raise RuntimeError('output_limit')
    return r
def save():
    (out/('comparison-boundary.json' if boundary else 'comparison-details.json' if details else 'comparison.json')).write_text(json.dumps({'owner':owner,'endpoint':endpoint,'imageId':runtime['imageId'],'groups':groups,'modelCalls':0},indent=2),encoding='utf-8')
for variant in (('bubblewrap',) if details or boundary else ('baseline','userns','bubblewrap')):
    profile=out/('moby-default.json' if variant=='baseline' else variant+'.json')
    expected=review['baselineHash'] if variant=='baseline' else next(v['sha256'] for v in review['variants'] if v['name']==variant)
    assert hashlib.sha256(profile.read_bytes()).hexdigest()==expected
    name='cortexos-bench-'+str(uuid.uuid4())
    group={'variant':variant,'name':name,'owner':owner,'profileHash':expected,'clean':False}
    groups.append(group);save()
    try:
        args=['create','--pull=never','--name',name,'--network=none','--read-only','--cpus=1','--memory=268435456','--memory-swap=268435456','--pids-limit=64','--cap-drop=ALL','--security-opt=no-new-privileges','--security-opt=seccomp='+str(profile),'--user=65534:65534','--label=com.cortexos.validation='+owner,'--label=com.cortexos.purpose=harness_claude_compatibility','--tmpfs=/tmp:rw,noexec,nosuid,size=16m,mode=1777','--tmpfs=/workspace:rw,noexec,nosuid,nodev,size=2m,uid=65534,gid=65534','--tmpfs=/home/cortexos:rw,noexec,nosuid,nodev,size=4m,mode=0700,uid=65534,gid=65534','--shm-size=8m','--ulimit=nofile=64:64','--ulimit=core=0:0','--ulimit=fsize=1048576:1048576','--log-driver=none','--no-healthcheck','--workdir=/workspace','--entrypoint=python3','-i',runtime['imageId'],'-I','-S','-u','-c',bootstrap]
        r=command(args);assert r.returncode==0,r.stderr
        config=json.loads(command(['inspect',name,'--format','{{json .}}']).stdout)
        assert config['Config']['Labels']['com.cortexos.validation']==owner and config['Config']['Image']==runtime['imageId']
        group['configuration']=config
        r=command(['start','--attach','--interactive',name],payload,30)
        group['exitCode']=r.returncode;group['stderr']=r.stderr.decode()
        group['observation']=json.loads(r.stdout) if r.returncode==0 else {'stdout':r.stdout.decode(errors='replace')}
    except Exception as e:
        group['error']=str(e)
    finally:
        found=command(['inspect',name,'--format','{{json .}}'])
        if found.returncode==0:
            config=json.loads(found.stdout)
            assert config['Config']['Labels']['com.cortexos.validation']==owner and config['Config']['Image']==runtime['imageId']
            assert command(['rm','--force',name]).returncode==0
        group['clean']=not command(['container','ls','-a','--filter','name=^/'+name+'$','--format','{{.Names}}']).stdout.strip()
        save()
    print(json.dumps({'variant':variant,'clean':group['clean'],'exitCode':group.get('exitCode'),'error':group.get('error')}),flush=True)

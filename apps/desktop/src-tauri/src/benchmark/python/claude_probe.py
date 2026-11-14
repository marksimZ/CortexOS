"""Offline program checks and optional trusted Bash fixture. No model or user project."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

EXPECTED = '4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658'

def run(arguments):
    try:
        result = subprocess.run(arguments, stdin=subprocess.DEVNULL, capture_output=True, timeout=8,
                                env={'PATH':os.environ['PATH'],'HOME':'/home/cortexos','CLAUDE_CONFIG_DIR':'/home/cortexos/.claude',
                                     'DISABLE_AUTOUPDATER':'1','DISABLE_UPDATES':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1','CI':'1'})
        if len(result.stdout) + len(result.stderr) > 24000:
            return {'reason':'output_limit'}
        return {'exitCode':result.returncode,'stdout':result.stdout.decode('utf-8',errors='replace'),'stderr':result.stderr.decode('utf-8',errors='replace')}
    except subprocess.TimeoutExpired:
        return {'reason':'timeout'}

def main():
    with open('/opt/claude/bin/claude','rb') as source:
        digest = hashlib.file_digest(source,'sha256').hexdigest()
    if digest != EXPECTED:
        return {'protocol':2,'binaryHash':digest,'modelCalls':0,'reason':'binary_mismatch'}
    os.makedirs('/home/cortexos/.claude',mode=0o700,exist_ok=True)
    status = dict(line.split(':',1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
    memory_events = Path('/sys/fs/cgroup/memory.events')
    observed = {'protocol':2,'probeVersion':'claude-program-2','binaryHash':digest,'python':list(sys.version_info[:2]),
                'version':run(['/opt/claude/bin/claude','--version']),
                'help':run(['/opt/claude/bin/claude','--help']),
                'parser':{'control':run(['/opt/claude/bin/claude','--bare','--print','--cortexos-invalid-option']),
                          'budget':run(['/opt/claude/bin/claude','--bare','--print','--max-turns','1','--cortexos-invalid-option'])},
                'dependencies':{name:shutil.which(name) for name in ('bwrap','socat')},
                'namespace':run(['python3','-I','-S','-c',"import ctypes,json; c=ctypes.CDLL(None,use_errno=True); r=c.unshare(0x10000000); print(json.dumps({'newUserNamespace':r==0,'errno':ctypes.get_errno() if r!=0 else 0}))"]),
                'container':{'uid':os.getuid(),'capEff':status['CapEff'].strip(),'capBnd':status['CapBnd'].strip(),'noNewPrivileges':status['NoNewPrivs'].strip(),'seccomp':status['Seccomp'].strip(),'dockerSocket':Path('/var/run/docker.sock').exists()},
                'modelCalls':0,'taskExecution':'Not Run','cliSandboxExecution':'Not Run','turnLimitExecution':'Not Run'}
    if globals().get('RUN_SANDBOX'):
        available=all(observed['dependencies'].values()) and '"newUserNamespace": true' in observed['namespace'].get('stdout','')
        if available:
            scope={'__name__':'cortexos_offline_probe'}
            exec(compile(SANDBOX_SOURCE,'<fixed-claude-sandbox-probe>','exec'),scope)
            observed['sandbox']=scope['observe']()
            observed['cliSandboxExecution']='Trusted offline fixture'
        else:
            observed['sandbox']={'passed':False,'reason':'sandbox_conditions_not_met'}
    observed['memoryEvents'] = memory_events.read_text() if memory_events.exists() else None
    return observed

try:
    print(json.dumps(main(),ensure_ascii=False))
except BaseException:
    print(json.dumps({'protocol':2,'reason':'claude_probe_setup_failed','modelCalls':0}))
    raise SystemExit(1)

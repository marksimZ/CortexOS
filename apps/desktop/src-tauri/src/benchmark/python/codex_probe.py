"""Fixed offline CLI probe, never an agent session or a model request."""
import hashlib
import json
import os
import socket
import subprocess

EXPECTED = {
    'bin/codex': 'b8a11fedf24b2dfb59a70c9e52b030f9a508dcd49cd2dd0d41956f4c785156b8',
    'codex-resources/bwrap': '77360cb751ccedc5971391444ac86a8a33c15b04d6b4a6fe45f5d25496e62c4c',
    'codex-path/rg': 'e62198eb19b136b88c330af83647b5a962cb99b6b1f066758568f12de1974849',
    'codex-package.json': 'a734b829e73b56b59a1ed45e8669785c8e3d23290e60f40f4b5174f68e8ea7cd',
}

def run(arguments):
    try:
        result = subprocess.run(arguments, stdin=subprocess.DEVNULL, capture_output=True, timeout=8)
        if len(result.stdout) + len(result.stderr) > 24000:
            return {'reason': 'output_limit'}
        return {'exitCode': result.returncode, 'stdout': result.stdout.decode('utf-8', errors='replace'), 'stderr': result.stderr.decode('utf-8', errors='replace')}
    except subprocess.TimeoutExpired:
        return {'reason': 'timeout'}

def main():
    os.makedirs(os.environ['CODEX_HOME'], mode=0o700, exist_ok=True)
    hashes = {}
    for relative in EXPECTED:
        with open('/opt/codex/' + relative, 'rb') as binary:
            hashes[relative] = hashlib.file_digest(binary, 'sha256').hexdigest()
    digest = hashes['bin/codex']
    if hashes != EXPECTED:
        return {'protocol': 2, 'binaryHash': digest, 'fileHashes': hashes, 'runtimeFilesVerified': False, 'reason': 'binary_mismatch', 'modelCalls': 0}
    probe = """import ctypes,errno,json,os,socket
from pathlib import Path
def denied_write(path):
    try:
        Path(path).write_text('forbidden')
    except OSError as problem:
        return {'denied':problem.errno in (errno.EPERM,errno.EACCES,errno.EROFS),'errno':problem.errno}
    return {'denied':False}
Path('/workspace/allowed.txt').write_text('allowed')
outside=denied_write('/tmp/outside-workspace.txt')
system=denied_write('/usr/local/cortexos-probe-forbidden')
libc=ctypes.CDLL(None,use_errno=True)
mounted=libc.mount(None,b'/tmp',b'tmpfs',0,None)
mount_errno=ctypes.get_errno() if mounted!=0 else 0
bpf=libc.syscall(321,-1,None,0)
bpf_errno=ctypes.get_errno() if bpf!=0 else 0
network_denied=False
try:
    with socket.socket() as connection:
        connection.settimeout(0.25)
        connection.connect(('127.0.0.1',LISTENER_PORT))
except OSError:
    network_denied=True
status=dict(line.split(':',1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
checks={'insideWrite':Path('/workspace/allowed.txt').read_text()=='allowed','outsideDenied':outside['denied'],'readonlyRootDenied':system['denied'],'networkDenied':network_denied,'mountDenied':mounted==-1 and mount_errno==errno.EPERM,'bpfDenied':bpf==-1 and bpf_errno==errno.EPERM,'zeroCapabilities':int(status['CapEff'].strip(),16)==0 and int(status['CapBnd'].strip(),16)==0,'noNewPrivileges':status['NoNewPrivs'].strip()=='1','seccompActive':status['Seccomp'].strip()=='2','noDockerSocket':not Path('/var/run/docker.sock').exists()}
print(json.dumps({**checks,'uid':os.getuid(),'outsideErrno':outside.get('errno'),'systemErrno':system.get('errno'),'mountErrno':mount_errno,'bpfErrno':bpf_errno}))
raise SystemExit(0 if all(checks.values()) else 1)
"""
    with socket.socket() as listener:
        listener.bind(('127.0.0.1',0))
        listener.listen(2)
        port=listener.getsockname()[1]
        # Prove the comparison endpoint is reachable in the outer container.
        with socket.create_connection(('127.0.0.1',port),timeout=1):
            pass
        sandbox=run(['codex','sandbox','-c','sandbox_mode="workspace-write"','-c','sandbox_workspace_write.exclude_slash_tmp=true','-c','sandbox_workspace_write.exclude_tmpdir_env_var=true','-c','sandbox_workspace_write.network_access=false','--','python3','-I','-S','-c',probe.replace('LISTENER_PORT',str(port))])
    return {'protocol': 2, 'probeVersion': 'codex-sandbox-2', 'binaryHash': digest, 'fileHashes': hashes, 'runtimeFilesVerified': True,
            'version': run(['codex', '--version']),
            'execHelp': run(['codex', 'exec', '--help']),
            'sandbox': sandbox,
            'modelCalls': 0}

try:
    result = main()
except BaseException:
    result = {'protocol': 2, 'reason': 'probe_setup_failed', 'modelCalls': 0}
print(json.dumps(result, ensure_ascii=False))

"""Approved tool preparation only. Wheel download in a disposable container; offline installation."""
import argparse
import email
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request
import uuid
import zipfile
from prepare_claude import guard, ENDPOINT

VERSION = '0.86.2'
HASH = '64f6a0c66c9f4633ad9f479bca3e64ebcba02b9da03c6b604b74a44736b2416e'
LIMIT = 1000000000
ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / 'artifacts/harness-aider-runtime'

def docker(*args, timeout=600):
    try:
        result = subprocess.run(['docker', '--host', ENDPOINT, *args], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)
    except subprocess.TimeoutExpired as error:
        with (CACHE/'last-command.log').open('a',encoding='utf-8') as log:
            log.write('\nTIMEOUT '+json.dumps(args)+'\n')
            for output in (error.stdout,error.stderr):
                if output: log.write(output.decode('utf-8',errors='replace') if isinstance(output,bytes) else output)
        raise
    with (CACHE/'last-command.log').open('a',encoding='utf-8') as log: log.write('\n'+json.dumps(args)+'\n'+result.stdout+result.stderr)
    if result.returncode: raise RuntimeError('Docker preparation failed; see last-command.log')
    return result.stdout.strip()

def copy_wheels(name, destination):
    # Docker cp reads the root filesystem, not this tmpfs. Export from inside it.
    archive=CACHE/'wheels.tar'; guard(archive)
    code="import pathlib,tarfile,sys; p=list(pathlib.Path('/cache/wheels').iterdir()); assert p and all(f.is_file() and not f.is_symlink() and f.suffix=='.whl' for f in p); assert sum(f.stat().st_size for f in p)<=450000000; t=tarfile.open(fileobj=sys.stdout.buffer,mode='w|'); [t.add(f,arcname=f.name,recursive=False) for f in p]; t.close()"
    with archive.open('wb') as out:
        result=subprocess.run(['docker','--host',ENDPOINT,'exec',name,'python','-c',code],stdout=out,stderr=subprocess.PIPE,timeout=60)
    if result.returncode: raise RuntimeError('Wheel export failed: '+result.stderr.decode('utf-8',errors='replace'))
    if archive.stat().st_size>LIMIT//2: raise RuntimeError('Export cache exceeds approved combined cache bound')
    destination.mkdir()
    with tarfile.open(archive) as tar:
        seen=set()
        for entry in tar:
            if not entry.isfile() or entry.name in seen or '/' in entry.name or '\\' in entry.name or ':' in entry.name or not entry.name.endswith('.whl'): raise RuntimeError('Invalid wheel archive entry')
            seen.add(entry.name); target=destination/entry.name; guard(target)
            with tar.extractfile(entry) as source, target.open('xb') as output: shutil.copyfileobj(source,output)
    archive.unlink()

def metadata(name, version):
    url = f'https://pypi.org/pypi/{name}/{version}/json'
    with urllib.request.urlopen(url, timeout=30) as response:
        raw = response.read(2000001)
    if len(raw)>2000000: raise RuntimeError('Metadata limit')
    return json.loads(raw)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-wheels',action='store_true',help='Recheck all cached wheels against official metadata, then rebuild offline.')
    options=parser.parse_args()
    guard(CACHE); CACHE.mkdir(exist_ok=True)
    for f in ['last-command.log','runtime.json','image.id','requirements.lock','Dockerfile','wheels','wheel-manifest.json']:
        guard(CACHE/f)
    if (CACHE/'runtime.json').exists(): raise RuntimeError('Prepared evidence exists; do not overwrite it')
    tag='python:3.12-slim-bookworm'
    docker('pull','--platform=linux/amd64',tag)
    base=json.loads(docker('image','inspect',tag))[0]
    if base['Os']!='linux' or base['Architecture']!='amd64': raise RuntimeError('Wrong base platform')
    # Resolve once; offline build below pins this content digest, never a moving tag.
    digest=next(d for d in base['RepoDigests'] if d.startswith('python@sha256:'))
    name='cortexos-aider-prepare-'+str(uuid.uuid4())
    wheels=CACHE/'wheels'
    if not options.reuse_wheels:
        if wheels.exists(): raise RuntimeError('Wheel cache exists; inspect partial evidence before resuming')
        try:
            docker('create','--name',name,'--pull=never','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges:true','--cpus=1','--memory=1g','--pids-limit=64','--tmpfs=/cache:rw,nosuid,nodev,size=1000000000','--env=HOME=/cache','--env=TMPDIR=/cache','--env=PIP_CONFIG_FILE=/dev/null','--env=PIP_DISABLE_PIP_VERSION_CHECK=1',base['Id'],'python','-c','import time; time.sleep(600)')
            docker('start',name)
            docker('exec',name,'python','-m','pip','download','--no-cache-dir','--only-binary=:all:','--index-url=https://pypi.org/simple','--dest=/cache/wheels',f'aider-chat=={VERSION}')
            state=json.loads(docker('inspect',name))[0]['State']
            if not state['Running'] or state['OOMKilled']: raise RuntimeError('Dependency resolution failed or exceeded preparation memory')
            copy_wheels(name,wheels)
        finally:
            docker('rm','--force',name,timeout=20)
    files=list(wheels.iterdir())
    if not files or sum(f.stat().st_size for f in files)>LIMIT: raise RuntimeError('Approved cache limit exceeded')
    evidence=[]; lock=[]
    for file in sorted(files):
        guard(file)
        if file.suffix!='.whl': raise RuntimeError('Only prebuilt wheels permitted')
        with zipfile.ZipFile(file) as wheel:
            names=[n for n in wheel.namelist() if n.endswith('.dist-info/METADATA')]
            if len(names)!=1: raise RuntimeError('Invalid wheel metadata')
            info=email.message_from_bytes(wheel.read(names[0]))
        name,version=info['Name'],info['Version']
        if not name or not version or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.+' for c in name+version): raise RuntimeError('Unsafe dependency identity')
        release=metadata(name,version)
        entry=next((f for f in release['urls'] if f['filename']==file.name),None)
        with file.open('rb') as stream: actual=hashlib.file_digest(stream,'sha256').hexdigest()
        if not entry or entry['digests']['sha256']!=actual or entry['size']!=file.stat().st_size or entry.get('yanked'): raise RuntimeError('Official wheel metadata mismatch: '+file.name)
        if name.lower().replace('_','-')=='aider-chat' and (version!=VERSION or actual!=HASH): raise RuntimeError('Approved Aider release mismatch')
        lock.append(f'{name}=={version} --hash=sha256:{actual}')
        evidence.append({'name':name,'version':version,'file':file.name,'sha256':actual,'size':entry['size'],'url':entry['url']})
    if sum(e['name'].lower().replace('_','-')=='aider-chat' for e in evidence)!=1: raise RuntimeError('Exactly one approved Aider wheel required')
    (CACHE/'requirements.lock').write_text('\n'.join(lock)+'\n',encoding='utf-8')
    (CACHE/'wheel-manifest.json').write_text(json.dumps(evidence,indent=2),encoding='utf-8')
    shutil.copyfile(ROOT/'scripts/harness/aider.Dockerfile',CACHE/'Dockerfile')
    docker('build','--network=none','--pull=false','--build-arg','BASE='+digest,'--iidfile',str(CACHE/'image.id'),'--tag','cortexos-aider:'+VERSION,str(CACHE))
    report={'version':VERSION,'wheelHash':HASH,'baseDigest':digest,'baseImageId':base['Id'],'imageId':(CACHE/'image.id').read_text().strip(),'dependencyCount':len(files),'wheelBytes':sum(f.stat().st_size for f in files),'lockHash':hashlib.sha256((CACHE/'requirements.lock').read_bytes()).hexdigest(),'modelCalls':0,'executionReady':False,'verification':'PyPI SHA256; no publisher signature claim'}
    (CACHE/'runtime.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))

if __name__=='__main__': main()

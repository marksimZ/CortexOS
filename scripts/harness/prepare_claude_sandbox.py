"""Build the approved dependency image; keeps all execution restrictions unchanged."""
import json
from pathlib import Path
import shutil
import subprocess
from prepare_claude import guard, ENDPOINT

repo=Path(__file__).resolve().parents[2]
parent=json.loads((repo/'artifacts/harness-claude-runtime/runtime.json').read_text())
cache=repo/'artifacts/harness-claude-sandbox'
guard(cache)
cache.mkdir(exist_ok=True)
for name in ['Dockerfile','image.id','runtime.json']:
    guard(cache/name)
tag='cortexos-claude:2.1.309'
actual=subprocess.run(['docker','--host',ENDPOINT,'image','inspect',tag,'--format','{{.Id}}'],capture_output=True,text=True,check=True,timeout=10).stdout.strip()
if actual!=parent['imageId']:
    raise SystemExit('The approved parent image no longer matches. Nothing built or downloaded.')
shutil.copyfile(repo/'scripts/harness/claude-sandbox.Dockerfile',cache/'Dockerfile')
subprocess.run(['docker','--host',ENDPOINT,'build','--pull=false','--iidfile',str(cache/'image.id'),'--tag','cortexos-claude:2.1.309-sandbox',str(cache)],check=True,timeout=300)
observed={'version':parent['version'],'parentImage':actual,'imageId':(cache/'image.id').read_text().strip(),
          'dependencies':'debian-trixie-bwrap-socat-1','modelCalls':0,'securityPolicy':'docker-default',
          'executionReady':False,'approval':'dependency_image_only'}
(cache/'runtime.json').write_text(json.dumps(observed,indent=2),encoding='utf-8')
print(json.dumps(observed,indent=2))

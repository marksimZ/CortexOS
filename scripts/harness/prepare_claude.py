"""Verify the approved cached native CLI and build offline. No model or installer."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

VERSION = '2.1.309'
CHECKSUM = '4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658'
SIZE = 293469552
FINGERPRINT = '31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE'
BASE = 'sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb'
ENDPOINT = 'npipe:////./pipe/dockerDesktopLinuxEngine'

def guard(path):
    if any(p.is_symlink() or p.is_junction() for p in (path, *path.parents)):
        raise SystemExit('Runtime cache must not contain redirected paths.')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--gpg', type=Path, required=True, help='Existing gpg.exe; this script does not install it.')
    parser.add_argument('--build', action='store_true')
    options = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    cache = repo / 'artifacts/harness-claude-runtime'
    guard(cache)
    for name in ['claude', 'manifest.json', 'manifest.json.sig', 'claude-code.asc']:
        file = cache / name
        guard(file)
        if not file.is_file() or file.stat().st_size > (SIZE if name == 'claude' else 131072):
            raise SystemExit('Approved runtime download missing or exceeds its bound: ' + name)
    release = json.loads((cache / 'manifest.json').read_text(encoding='utf-8'))
    platform = release['platforms']['linux-x64']
    if release['version'] != VERSION or platform != {'binary':'claude', 'checksum':CHECKSUM, 'size':SIZE}:
        raise SystemExit('Pinned official release manifest mismatch.')
    with (cache / 'claude').open('rb') as binary:
        if hashlib.file_digest(binary, 'sha256').hexdigest() != CHECKSUM or (cache / 'claude').stat().st_size != SIZE:
            raise SystemExit('Official CLI size/SHA-256 mismatch; nothing executed.')
    # A dedicated empty keyring; never reads personal gpg settings or retrieves keys.
    keyring = cache / 'verification-keyring'
    guard(keyring)
    keyring.mkdir(exist_ok=True)
    # Git's MSYS gpg treats native backslash paths as relative paths. Fixed names
    # in this guarded cache work with both MSYS and native gpg, without user config.
    prefix = [str(options.gpg.resolve(strict=True)), '--no-options', '--homedir', 'verification-keyring', '--batch', '--no-auto-key-retrieve', '--no-auto-key-import']
    subprocess.run([*prefix, '--import', 'claude-code.asc'], cwd=cache, check=True, capture_output=True, timeout=15)
    signature = subprocess.run([*prefix, '--status-fd=1', '--verify', 'manifest.json.sig', 'manifest.json'], cwd=cache, check=True, capture_output=True, text=True, timeout=15)
    valid = [s.split() for s in signature.stdout.splitlines() if s.startswith('[GNUPG:] VALIDSIG ')]
    if len(valid) != 1 or FINGERPRINT not in (valid[0][2], valid[0][-1]):
        raise SystemExit('Official release signature/fingerprint mismatch.')
    context = cache / 'context'
    guard(context)
    context.mkdir(exist_ok=True)
    for target in ['claude','Dockerfile']:
        guard(context / target)
    shutil.copyfile(cache / 'claude', context / 'claude')
    shutil.copyfile(repo / 'scripts/harness/claude.Dockerfile', context / 'Dockerfile')
    observed = {'version':VERSION,'url':f'https://downloads.claude.ai/claude-code-releases/{VERSION}/linux-x64/claude','binaryHash':CHECKSUM,'size':SIZE,'manifestSignature':'verified','signingKeyFingerprint':FINGERPRINT,'baseImage':BASE,'modelCalls':0}
    if options.build:
        local = subprocess.run(['docker','--host',ENDPOINT,'image','inspect','python:3.13-slim','--format','{{.Id}}'],capture_output=True,text=True,check=True,timeout=10).stdout.strip()
        if local != BASE:
            raise SystemExit('Existing Python base tag differs; no build or download performed.')
        guard(cache / 'image.id')
        subprocess.run(['docker','--host',ENDPOINT,'build','--network=none','--pull=false','--iidfile',str(cache / 'image.id'),'--tag',f'cortexos-claude:{VERSION}',str(context)],check=True,timeout=180)
        observed['imageId'] = (cache / 'image.id').read_text().strip()
    guard(cache / 'runtime.json')
    (cache / 'runtime.json').write_text(json.dumps(observed,indent=2),encoding='utf-8')
    print(json.dumps(observed,indent=2))

if __name__ == '__main__':
    main()

"""Prepare an already approved/downloaded official CLI archive; never calls a model."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

VERSION = '0.166.0'
INTEGRITY = 'CxDm3+D1cpbGJeHrdzINoG+3wn9BVncP4qpZaaNYkxe8iPtRJOYyCJnnPoVjWRIBvI9vPKbRoifsMSA5xDaUpw=='
BASE = 'sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb'
PREFIX = 'package/vendor/x86_64-unknown-linux-musl/'
FILES = ('bin/codex', 'codex-resources/bwrap', 'codex-path/rg', 'codex-package.json')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build', action='store_true', help='Build the approved image offline from the existing local base.')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    cache = repo / 'artifacts/harness-codex-runtime'
    archive = cache / f'codex-{VERSION}-linux-x64.tgz'
    def guard(path):
        if any(p.is_symlink() or p.is_junction() for p in (path, *path.parents)):
            raise SystemExit('Runtime cache must not contain redirected paths.')
    guard(archive)
    with archive.open('rb') as source:
        actual = base64.b64encode(hashlib.file_digest(source, 'sha512').digest()).decode('ascii')
    if actual != INTEGRITY:
        raise SystemExit('Official archive SHA-512 mismatch; nothing extracted or executed.')
    context = cache / 'context'
    guard(context)
    context.mkdir(exist_ok=True)
    hashes = {}
    with tarfile.open(archive, 'r:gz') as package:
        for relative in FILES:
            member = package.getmember(PREFIX + relative)
            if not member.isfile() or member.size > 512 * 1024 * 1024:
                raise SystemExit('Invalid runtime archive entry.')
            # Fixed names only: never extract archive-controlled paths or links.
            destination = context / relative
            guard(destination)
            destination.parent.mkdir(parents=True, exist_ok=True)
            with package.extractfile(member) as source, destination.open('wb') as target:
                while chunk := source.read(1024 * 1024):
                    target.write(chunk)
            with destination.open('rb') as source:
                hashes[relative] = hashlib.file_digest(source, 'sha256').hexdigest()
    dockerfile = repo / 'scripts/harness/codex.Dockerfile'
    guard(context / 'Dockerfile')
    (context / 'Dockerfile').write_bytes(dockerfile.read_bytes())
    manifest = {'version': VERSION, 'archiveUrl': f'https://registry.npmjs.org/@openai/codex/-/codex-{VERSION}-linux-x64.tgz', 'archiveIntegrity': 'sha512-' + actual, 'files': hashes, 'baseImage': BASE, 'modelCalls': 0}
    if args.build:
        guard(cache / 'image.id')
        local = subprocess.run(['docker', '--host', 'npipe:////./pipe/dockerDesktopLinuxEngine', 'image', 'inspect', 'python:3.13-slim', '--format', '{{.Id}}'], check=True, capture_output=True, text=True, timeout=10).stdout.strip()
        if local != BASE:
            raise SystemExit('The local Python base tag no longer matches the verified image.')
        subprocess.run(['docker', '--host', 'npipe:////./pipe/dockerDesktopLinuxEngine', 'build', '--network=none', '--pull=false', '--iidfile', str(cache / 'image.id'), '--tag', f'cortexos-codex:{VERSION}', str(context)], check=True, timeout=180)
        manifest['imageId'] = (cache / 'image.id').read_text().strip()
    guard(cache / 'runtime.json')
    (cache / 'runtime.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(manifest, ensure_ascii=False, indent=2))

if __name__ == '__main__':
    main()

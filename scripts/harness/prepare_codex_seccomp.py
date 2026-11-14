"""Prepare review-only profiles. This script never starts a container or changes policy."""
import copy
import hashlib
import json
from pathlib import Path

BASE_HASH = '6416b47770785a41ac59073cdc77d9fe98517df2799dc83ef207e622de3053f6'
ROOT = Path(__file__).resolve().parents[2] / 'artifacts/harness-codex-namespace'
raw = (ROOT / 'moby-default.json').read_bytes()
if hashlib.sha256(raw).hexdigest() != BASE_HASH:
    raise SystemExit('The official review baseline no longer matches its pinned hash.')
base = json.loads(raw)
if base['defaultAction'] != 'SCMP_ACT_ERRNO' or base['defaultErrnoRet'] != 1:
    raise SystemExit('Invalid default-deny baseline.')

# Only USER/MOUNT/UTS/IPC/PID/NET namespace flags; cgroup and all other bits stay denied.
unshare = {'names': ['unshare'], 'action': 'SCMP_ACT_ALLOW', 'args': [
    {'index': 0, 'value': 0xffffffff ^ 0x7c020000, 'valueTwo': 0, 'op': 'SCMP_CMP_MASKED_EQ'}]}
# The extra clone rule requires NEWUSER and excludes NEWCGROUP; ordinary clone stays as upstream.
clone = {'names': ['clone'], 'action': 'SCMP_ACT_ALLOW', 'includes': {'arches': ['amd64']}, 'args': [
    {'index': 0, 'value': 0x12000000, 'valueTwo': 0x10000000, 'op': 'SCMP_CMP_MASKED_EQ'}]}
filesystem = {'names': ['mount', 'umount2', 'pivot_root', 'chroot'], 'action': 'SCMP_ACT_ALLOW'}

manifest = {'source': json.loads((ROOT / 'source.json').read_text(encoding='utf-8-sig')),
            'baselineRelationship': 'Official Moby snapshot, not a claimed export of the installed engine filter',
            'modelCalls': 0, 'applied': False, 'variants': []}
for name, extra in [('userns', [unshare]), ('bubblewrap', [unshare, clone, filesystem])]:
    candidate = copy.deepcopy(base)
    candidate['syscalls'].extend(extra)
    encoded = (json.dumps(candidate, separators=(',', ':')) + '\n').encode()
    (ROOT / f'{name}.json').write_bytes(encoded)
    manifest['variants'].append({'name': name, 'sha256': hashlib.sha256(encoded).hexdigest(), 'addedRules': extra})
(ROOT / 'review.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(json.dumps(manifest, indent=2))

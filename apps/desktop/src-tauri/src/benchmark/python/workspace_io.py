"""Trusted file transport only. Never imports or executes submitted project code."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys

ROOT = Path('/workspace')

def digest(data):
    return hashlib.sha256(data).hexdigest()

def checked_path(text):
    path = PurePosixPath(text)
    if not text or len(text) > 512 or path.is_absolute() or '\\' in text or ':' in text or any(p in ('', '.', '..') for p in text.split('/')):
        raise ValueError('workspace_invalid_path')
    return ROOT.joinpath(*path.parts)

def read_file(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > limit:
            raise ValueError('workspace_invalid_file')
        data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError('workspace_file_limit')
        return data

PROTECTED = ('.git', '.agents', '.codex', '.aws')

def collect(protected=False):
    raw = read_file(ROOT / 'task.json', 65536)
    task = json.loads(raw)
    project = task.get('version') == 'python-project-1'
    targets = task['files'] if project else [task['target']['path']]
    for target in targets:
        checked_path('project/' + target)
    expected = {'task.json', *('project/' + target for target in targets)}
    allowed_dirs = {'.'}
    if protected:
        allowed_dirs.update(PROTECTED)
        if any(not (ROOT / name).is_dir() or (ROOT / name).is_symlink() or any((ROOT / name).iterdir()) for name in PROTECTED):
            raise ValueError('workspace_protected_directory_changed')
    for file in expected:
        allowed_dirs.update(p.as_posix() for p in PurePosixPath(file).parents)
    files, contents = [], {}
    for base, dirs, names in os.walk(ROOT, followlinks=False):
        info = Path(base).stat()
        if info.st_uid != 0 or stat.S_IMODE(info.st_mode) != 0o555:
            raise ValueError('workspace_directory_permissions')
        if len(files) + len(dirs) + len(names) > 80:
            raise ValueError('workspace_entry_limit')
        for name in dirs:
            directory = Path(base) / name
            if directory.is_symlink() or directory.relative_to(ROOT).as_posix() not in allowed_dirs:
                raise ValueError('workspace_unexpected_directory')
        for name in names:
            file = Path(base) / name
            relative = file.relative_to(ROOT).as_posix()
            if relative not in expected or file.is_symlink():
                raise ValueError('workspace_unexpected_file')
            data = read_file(file, 65536 if relative == 'task.json' else 24000)
            files.append({'path': relative, 'hash': digest(data), 'mode': stat.S_IMODE(file.stat().st_mode), 'uid':file.stat().st_uid})
            if relative != 'task.json':
                contents[relative.removeprefix('project/')] = data.decode('utf-8')
    if {f['path'] for f in files} != expected:
        raise ValueError('workspace_missing_file')
    if project and sum(len(code.encode('utf-8')) for code in contents.values()) > 24000:
        raise ValueError('workspace_file_limit')
    result = {'protocol': 2 if project else 1, 'python': list(sys.version_info[:2]), 'taskHash': digest(raw), 'files': sorted(files, key=lambda f:f['path'])}
    if project:
        result['contents'] = contents
    else:
        result['code'] = contents[targets[0]]
    if protected:
        result['protectedDirectories'] = list(PROTECTED)
    return result

def main():
    payload = json.loads(sys.stdin.buffer.read(131073))
    if payload.get('directoryPolicy') not in (None, 'codex-protected-empty-1', 'claude-protected-empty-1', 'aider-single-file-1'):
        raise ValueError('workspace_invalid_directory_policy')
    protected = payload.get('directoryPolicy') in ('codex-protected-empty-1', 'claude-protected-empty-1')
    global PROTECTED
    if payload.get('directoryPolicy') == 'claude-protected-empty-1':
        PROTECTED = (*PROTECTED, '.claude')
    version = (3, 12) if payload.get('directoryPolicy') == 'aider-single-file-1' else (3, 13)
    if sys.version_info[:2] != version:
        raise ValueError('python_runtime_mismatch')
    if payload['op'] == 'init':
        bundle = payload['bundle']
        task = json.loads(bundle['taskJson'])
        project = task.get('version') == 'python-project-1'
        contents = bundle['originalFiles'] if project else {task['target']['path']: bundle['originalCode']}
        writes = task['writePaths'] if project else contents
        if project and set(contents) != set(task['files']):
            raise ValueError('workspace_missing_file')
        for name, content in contents.items():
            target = checked_path('project/' + name)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content.encode('utf-8'))
            os.chmod(target, 0o666 if name in writes else 0o444)
        (ROOT / 'task.json').write_bytes(bundle['taskJson'].encode('utf-8'))
        if protected:
            for name in PROTECTED:
                (ROOT / name).mkdir()
        os.chmod(ROOT / 'task.json', 0o444)
        for base, dirs, _ in os.walk(ROOT, topdown=False):
            for directory in dirs:
                os.chmod(Path(base) / directory, 0o555)
        os.chmod(ROOT, 0o555)
    elif payload['op'] == 'write':
        before = collect(protected)
        if before['protocol'] != 1:
            raise ValueError('workspace_invalid_operation')
        if before['taskHash'] != payload['taskHash']:
            raise ValueError('workspace_task_changed')
        task = json.loads(read_file(ROOT / 'task.json', 65536))
        target = checked_path('project/' + task['target']['path'])
        content = payload['code'].encode('utf-8')
        if len(content) > 24000:
            raise ValueError('workspace_file_limit')
        fd = os.open(target, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(content)
    elif payload['op'] != 'collect':
        raise ValueError('workspace_invalid_operation')
    print(json.dumps(collect(protected), ensure_ascii=False))

try:
    main()
except BaseException as problem:
    code = str(problem) if isinstance(problem, ValueError) and str(problem).startswith(('workspace_', 'python_')) else 'workspace_file_operation_failed'
    print(json.dumps({'protocol':1, 'error':code}))
    sys.exit(1)

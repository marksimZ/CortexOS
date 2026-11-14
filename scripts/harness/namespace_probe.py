"""Fixed in-container diagnostics. Does not invoke models or change host settings."""
import ctypes
import errno
import json
import os
from pathlib import Path

libc = ctypes.CDLL(None, use_errno=True)
libc.unshare.argtypes = [ctypes.c_int]
libc.unshare.restype = ctypes.c_int

def attempt(flags):
    read_fd, write_fd = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(read_fd)
        result = libc.unshare(flags)
        code = ctypes.get_errno() if result != 0 else 0
        os.write(write_fd, json.dumps({'result': result, 'errno': code, 'error': errno.errorcode.get(code)}).encode())
        os._exit(0)
    os.close(write_fd)
    with os.fdopen(read_fd) as source:
        observation = json.loads(source.read())
    os.waitpid(pid, 0)
    return observation

def read(path):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return None

status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
print(json.dumps({'uid': os.getuid(), 'kernel': os.uname().release,
                  'status': {key: status.get(key, '').strip() for key in ('CapEff', 'CapBnd', 'NoNewPrivs', 'Seccomp', 'Seccomp_filters')},
                  'maxUserNamespaces': read('/proc/sys/user/max_user_namespaces'),
                  'unprivilegedUsernsClone': read('/proc/sys/kernel/unprivileged_userns_clone'),
                  'lsm': read('/proc/self/attr/current'),
                  'unshareNoFlags': attempt(0), 'unshareUser': attempt(0x10000000), 'modelCalls': 0}, indent=2))

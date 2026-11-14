"""Container-local relay. No host address, credentials, answer or grader data."""
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

LIMIT = 65536
lock = threading.Lock()
count = 0

def emit(kind, **data):
    with lock:
        print(json.dumps({'kind': kind, **data}, ensure_ascii=False), flush=True)

class Relay(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        global count
        if self.path != '/v1/responses' or self.headers.get('Transfer-Encoding') or self.headers.get('Content-Encoding'):
            self.send_error(400)
            return
        try:
            size = int(self.headers.get('Content-Length', '0'))
            if count >= 4:
                emit('error', reason='request_budget_exceeded')
                raise ValueError('request_limit')
            if size <= 0 or size > LIMIT:
                raise ValueError('request_limit')
            count += 1
            emit('request', id=count, body=json.loads(self.rfile.read(size)))
            raw = sys.stdin.buffer.readline(LIMIT * 3 + 1)
            if not raw or len(raw) > LIMIT * 3:
                raise ValueError('response_limit')
            reply = json.loads(raw)
            if reply.get('id') != count or not isinstance(reply.get('sse'), str):
                raise ValueError('invalid_reply')
            data = reply['sse'].encode('utf-8')
            if len(data) > LIMIT * 3:
                raise ValueError('response_limit')
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except BaseException:
            self.send_error(400, 'controlled_channel_stopped')

def pump(stream, kind):
    total = 0
    while True:
        raw = stream.readline(LIMIT + 1)
        if not raw:
            return
        total += len(raw)
        if len(raw) > LIMIT or total > 262144:
            emit('error', reason='cli_output_limit')
            return
        emit(kind, raw=raw.decode('utf-8', errors='replace').rstrip('\r\n'))

def main():
    raw = sys.stdin.buffer.readline(LIMIT + 1)
    if len(raw) > LIMIT:
        raise ValueError('payload_limit')
    payload = json.loads(raw)
    os.makedirs('/home/cortexos/.codex', mode=0o700, exist_ok=True)
    instructions = ('Implement the task using the provided workspace. Edit only the approved existing target paths in place. '
                    'Directories and task.json are read-only; do not create or rename files. '
                    'Use the available command tool. Do not use network, install tools, request elevated permissions, '
                    'or try to access hidden tests. Stop when the approved changes implement the task.')
    Path('/home/cortexos/instructions.md').write_text(instructions, encoding='utf-8')
    server = http.server.HTTPServer(('127.0.0.1', 0), Relay)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    options = {
        'model_provider': 'cortexos_local',
        'model_providers.cortexos_local.name': 'CortexOS controlled local channel',
        'model_providers.cortexos_local.base_url': f'http://127.0.0.1:{server.server_port}/v1',
        'model_providers.cortexos_local.wire_api': 'responses',
        'model_providers.cortexos_local.request_max_retries': 0,
        'model_providers.cortexos_local.stream_max_retries': 0,
        'model_instructions_file': '/home/cortexos/instructions.md',
        'model_context_window': 4096,
        'sandbox_workspace_write.exclude_slash_tmp': True,
        'sandbox_workspace_write.exclude_tmpdir_env_var': True,
        'sandbox_workspace_write.network_access': False,
        'features.multi_agent': False, 'features.goals': False,
        'features.enable_request_compression': False,
        'analytics.enabled': False, 'web_search': 'disabled',
    }
    args = ['codex', '--no-daemon', '-a', 'on-request']
    for key, value in options.items():
        args.extend(['-c', key + '=' + json.dumps(value)])
    args.extend(['exec', '--ignore-user-config', '--ignore-rules', '--ephemeral',
                 '--skip-git-repo-check', '--json', '--color', 'never', '--sandbox', 'workspace-write',
                 '--model', payload['model'], payload['prompt']])
    emit('configuration', arguments=args, approval='exec_rejects_escalation', modelCalls=0)
    env = {'PATH': os.environ['PATH'], 'HOME': '/home/cortexos', 'CODEX_HOME': '/home/cortexos/.codex',
           'TERM': 'dumb', 'NO_COLOR': '1', 'OTEL_SDK_DISABLED': 'true'}
    child = subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             cwd='/workspace', env=env)
    workers = [threading.Thread(target=pump, args=(child.stdout, 'cli')),
               threading.Thread(target=pump, args=(child.stderr, 'diagnostic'))]
    for worker in workers:
        worker.start()
    code = child.wait(timeout=180)
    for worker in workers:
        worker.join(timeout=2)
    emit('exit', exitCode=code)
    server.shutdown()

try:
    main()
except BaseException as problem:
    emit('error', reason='session_controller_error', exception=type(problem).__name__)
    raise SystemExit(1)

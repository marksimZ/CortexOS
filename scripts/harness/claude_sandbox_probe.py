"""A fixed offline fixture drives Claude's real Bash tool. No model is contacted."""
import errno
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import subprocess
import threading

EXPECTED='4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658'

def observe():
    with open('/opt/claude/bin/claude','rb') as binary:
        if hashlib.file_digest(binary,'sha256').hexdigest()!=EXPECTED:
            raise RuntimeError('binary_mismatch')
    home=Path('/home/cortexos')
    (home/'.claude').mkdir(exist_ok=True)
    outside=Path('/tmp/cortexos-outside')
    outside.mkdir(exist_ok=True)
    (outside/'control.txt').write_text('outer-writable')
    protected=home/'protected'
    protected.mkdir(exist_ok=True)
    (protected/'control.txt').write_text('outer-writable')
    command='python3 -I -S /tmp/cortexos-isolation-probe.py'
    requests=[]
    results=[]
    server_errors=[]

    class Handler(BaseHTTPRequestHandler):
        def log_message(self,*args):
            pass
        def do_POST(self):
            size=int(self.headers.get('Content-Length','0'))
            if size<1 or size>1048576:
                self.send_error(413);return
            body=json.loads(self.rfile.read(size))
            if self.path.startswith('/v1/messages/count_tokens'):
                encoded=b'{"input_tokens":1}'
                self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(encoded)));self.end_headers();self.wfile.write(encoded);return
            if self.path.split('?')[0]!='/v1/messages' or len(requests)>=2:
                server_errors.append('unexpected_fixture_request');self.send_error(400);return
            calls=[block for message in body.get('messages',[]) for block in (message.get('content',[]) if isinstance(message.get('content'),list) else []) if block.get('type')=='tool_result']
            results.extend(calls)
            requests.append({'path':self.path,'model':body.get('model'),'tools':[t.get('name') for t in body.get('tools',[])],'toolResults':calls})
            first=len(requests)==1
            content={'type':'tool_use','id':'toolu_cortexos_fixed_probe','name':'Bash','input':{}} if first else {'type':'text','text':''}
            delta={'type':'input_json_delta','partial_json':json.dumps({'command':command,'timeout':5000,'description':'Run the fixed offline isolation check'})} if first else {'type':'text_delta','text':'Offline fixture completed.'}
            events=[('message_start',{'type':'message_start','message':{'id':'msg_cortexos_fixture_'+str(len(requests)),'type':'message','role':'assistant','model':'claude-sandbox-fixture','content':[],'stop_reason':None,'stop_sequence':None,'usage':{'input_tokens':1,'output_tokens':0}}}),
                    ('content_block_start',{'type':'content_block_start','index':0,'content_block':content}),
                    ('content_block_delta',{'type':'content_block_delta','index':0,'delta':delta}),
                    ('content_block_stop',{'type':'content_block_stop','index':0}),
                    ('message_delta',{'type':'message_delta','delta':{'stop_reason':'tool_use' if first else 'end_turn','stop_sequence':None},'usage':{'output_tokens':1}}),
                    ('message_stop',{'type':'message_stop'})]
            encoded=''.join('event: '+kind+'\ndata: '+json.dumps(data)+'\n\n' for kind,data in events).encode()
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Content-Length',str(len(encoded)));self.end_headers();self.wfile.write(encoded)

    server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    port=server.server_address[1]
    server.daemon_threads=True
    thread=threading.Thread(target=server.serve_forever,daemon=True)
    thread.start()
    # The control proves this endpoint is reachable outside the inner sandbox.
    with socket.create_connection(('127.0.0.1',port),timeout=1):
        pass
    check='''import ctypes,errno,json,os,socket
from pathlib import Path
def denied(path):
 try:
  Path(path).write_text('forbidden')
  return False
 except OSError as e:return e.errno in (errno.EPERM,errno.EACCES,errno.EROFS)
Path('/workspace/allowed.txt').write_text('allowed')
libc=ctypes.CDLL(None,use_errno=True)
mount=libc.mount(None,b'/tmp',b'tmpfs',0,None)
mount_errno=ctypes.get_errno()
bpf=libc.syscall(321,-1,None,0)
bpf_errno=ctypes.get_errno()
network_denied=False
try:
 with socket.create_connection(('127.0.0.1',LISTENER_PORT),timeout=0.25):pass
except OSError:network_denied=True
status=dict(line.split(':',1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
def namespace_attempt(clone):
 read_fd,write_fd=os.pipe()
 pid=os.fork()
 if pid==0:
  os.close(read_fd)
  r=libc.syscall(56,0x10000000|17,None,None,None,0) if clone else libc.unshare(0x10000000)
  code=ctypes.get_errno() if r==-1 else 0
  if clone and r==0:os._exit(0)
  if clone and r>0:os.waitpid(r,0)
  os.write(write_fd,json.dumps({'denied':r==-1 and code==errno.EPERM,'errno':code}).encode())
  os._exit(0)
 os.close(write_fd)
 with os.fdopen(read_fd) as source:result=json.loads(source.read())
 os.waitpid(pid,0)
 return result
unshare=namespace_attempt(False)
clone=namespace_attempt(True)
checks={'insideWrite':Path('/workspace/allowed.txt').read_text()=='allowed','outsideDenied':denied('/tmp/cortexos-outside/control.txt'),'homeDenied':denied('/home/cortexos/protected/control.txt'),'readonlyRootDenied':denied('/usr/local/cortexos-forbidden'),'networkDenied':network_denied,'mountDenied':mount==-1 and mount_errno==errno.EPERM,'bpfDenied':bpf==-1 and bpf_errno==errno.EPERM,'noUsableCapabilities':all(int(status[k].strip(),16)==0 for k in ('CapEff','CapPrm','CapAmb')),'nonRoot':os.getuid()==65534,'nestedUserNamespaceDenied':unshare['denied'] and clone['denied'],'noNewPrivileges':status['NoNewPrivs'].strip()=='1','seccompActive':status['Seccomp'].strip()=='2','noDockerSocket':not Path('/var/run/docker.sock').exists()}
print(json.dumps({'protocol':'cortexos-claude-fixed-sandbox-2','checks':checks,'uid':os.getuid(),'capabilities':{k:status[k].strip() for k in ('CapEff','CapBnd','CapPrm','CapAmb')},'namespaceAttempts':{'unshare':unshare,'clone':clone},'modelCalls':0}))
raise SystemExit(0 if all(checks.values()) else 1)
'''
    Path('/tmp/cortexos-isolation-probe.py').write_text(check.replace('LISTENER_PORT',str(port)))
    settings={'sandbox':{'enabled':True,'failIfUnavailable':True,'allowUnsandboxedCommands':False,'enableWeakerNestedSandbox':False,'enableWeakerNetworkIsolation':False,'autoAllowBashIfSandboxed':False,'excludedCommands':[],
                         'network':{'allowedDomains':[],'allowAllUnixSockets':False},'filesystem':{'allowWrite':['/workspace'],'denyWrite':[str(outside),str(protected)]}},'permissions':{'defaultMode':'default'}}
    env={'PATH':os.environ['PATH'],'HOME':str(home),'CLAUDE_CONFIG_DIR':str(home/'.claude'),'SHELL':'/bin/bash','CI':'1','DISABLE_AUTOUPDATER':'1','DISABLE_UPDATES':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1',
         'ANTHROPIC_BASE_URL':'http://127.0.0.1:'+str(port),'ANTHROPIC_API_KEY':'cortexos-offline-fixture-not-a-real-key','CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS':'1'}
    args=['claude','--bare','--print','--verbose','--output-format','stream-json','--no-session-persistence','--max-turns','2','--settings',json.dumps(settings),'--setting-sources','','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--tools','Bash','--allowedTools','Bash('+command+')','--permission-mode','default','--model','claude-sandbox-fixture','Run the fixed offline isolation check once.']
    try:
        r=subprocess.run(args,stdin=subprocess.DEVNULL,capture_output=True,env=env,timeout=18)
        if len(r.stdout)+len(r.stderr)>48000:
            raise RuntimeError('output_limit')
        cli={'exitCode':r.returncode,'stdout':r.stdout.decode(errors='replace'),'stderr':r.stderr.decode(errors='replace')}
    except subprocess.TimeoutExpired:
        cli={'reason':'timeout'}
    finally:
        server.shutdown();server.server_close();thread.join(timeout=1)
    proof=None
    for result in results:
        content=result.get('content','')
        if isinstance(content,list):content='\n'.join(c.get('text','') for c in content)
        for line in content.splitlines():
            try:
                value=json.loads(line)
            except ValueError:
                continue
            if value.get('protocol')=='cortexos-claude-fixed-sandbox-2':proof=value
    unchanged=(outside/'control.txt').read_text()=='outer-writable' and (protected/'control.txt').read_text()=='outer-writable'
    passed=bool(proof and all(proof['checks'].values()) and unchanged and cli.get('exitCode')==0 and len(requests)==2 and not server_errors)
    memory=Path('/sys/fs/cgroup/memory.events')
    return {'protocol':'claude-sandbox-fixture-2','mode':'trusted_offline_fixture_not_model','settings':settings,'cli':cli,'requests':requests,'serverErrors':server_errors,'proof':proof,'outerControlsReachable':True,'protectedFilesUnchanged':unchanged,'passed':passed,'memoryEvents':memory.read_text() if memory.exists() else None,'modelCalls':0,'taskExecution':'Not Run'}

if __name__=='__main__':
    print(json.dumps(observe(),ensure_ascii=False))

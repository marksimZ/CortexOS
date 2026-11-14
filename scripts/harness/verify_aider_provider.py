"""Isolated fake HTTP Provider test; never starts Ollama or a model."""
import http.server,json,os,subprocess,threading
from pathlib import Path
requests=[]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def reply(self,body):
        data=json.dumps(body).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    def do_GET(self):
        requests.append({'path':self.path})
        if self.path=='/api/tags': self.reply({'models':[{'name':'fixture-only','model':'fixture-only','digest':'a'*64}]})
        elif self.path=='/api/ps':self.reply({'models':[]})
        elif self.path=='/api/version':self.reply({'version':'fixture'})
        else:self.send_error(404)
    def do_POST(self):
        n=int(self.headers.get('Content-Length','0'));assert 0<n<=65536
        body=json.loads(self.rfile.read(n));requests.append({'path':self.path,'body':body})
        assert self.path=='/api/chat' and body['model']=='fixture-only'
        assert body['options']=={'num_ctx':4096,'num_predict':512,'temperature':0}
        assert body['stream'] is False and body['keep_alive']==0
        self.reply({'model':'fixture-only','done':True,'done_reason':'stop','message':{'role':'assistant','content':'fixture edit'},'prompt_eval_count':8,'eval_count':2})
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
root=Path(__file__).resolve().parents[2];out=root/'artifacts/harness-aider-provider';out.mkdir(exist_ok=True)
env={**os.environ,'CORTEXOS_TEST_OLLAMA_URL':f'http://127.0.0.1:{server.server_port}','CORTEXOS_AIDER_PROVIDER_FIXTURE':'1'}
try:
    result=subprocess.run(['cargo','test','--offline','--manifest-path','apps/desktop/src-tauri/Cargo.toml','aider_provider_fixture','--','--ignored'],cwd=root,env=env,capture_output=True,timeout=180)
    (out/'test.log').write_bytes(result.stdout+result.stderr)
    (out/'requests.json').write_text(json.dumps(requests,indent=2),encoding='utf-8')
    assert result.returncode==0, 'See test.log'
    assert len([r for r in requests if r['path']=='/api/chat'])==1
    print(json.dumps({'passed':True,'modelCalls':0,'fakeChatRequests':1,'evidence':str(out)}))
finally:server.shutdown();server.server_close()

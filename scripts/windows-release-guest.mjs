// Run only from an explicitly mapped Windows Sandbox test input directory.
import assert from "node:assert/strict";
import { execFile, execFileSync, spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { once } from "node:events";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";

const input = "C:\\CortexOS-input";
const output = "C:\\CortexOS-output";
const mode = process.argv[2];
assert.equal(process.env.USERNAME, "WDAGUtilityAccount", "Requires a disposable Windows Sandbox account");
assert.ok(["fresh", "upgrade"].includes(mode));
const run = promisify(execFile);
const programDirectory = join(process.env.LOCALAPPDATA, "CortexOS");
const program = join(programDirectory, "cortexos-desktop.exe");
const uninstaller = join(programDirectory, "uninstall.exe");
const data = join(process.env.APPDATA, "com.cortexos.desktop");
const dbPath = join(data, "cortexos.sqlite3");
const port = 49491;
const report = { mode, cases: [] };
const digest = (name) => createHash("sha256").update(readFileSync(name)).digest("hex");
const evidence = () => writeFileSync(join(output, `${mode}.json`), JSON.stringify(report, null, 2) + "\n");
const record = (name, detail) => { report.cases.push({ name, result: "passed", detail }); evidence(); };
const metadata = JSON.parse(readFileSync(join(input, "candidate.json"), "utf8").replace(/^\uFEFF/, ""));
const old = join(input, "CortexOS_0.2.0_x64-setup.exe");
const candidate = join(input, metadata.filename);
assert.equal(digest(candidate), metadata.sha256.toLowerCase());
assert.equal(digest(old), "b3a18e39b590c2615b024293e172e434949b263daf37cd7d122ff8c71a9ed1eb");
assert.equal(existsSync(dbPath), false);
assert.equal(existsSync(program), false);
evidence();

async function waitFor(read, milliseconds = 30_000) {
  const until = Date.now() + milliseconds;
  let error;
  do {
    try { const result = await read(); if (result) return result; } catch (failure) { error = failure; }
    await delay(300);
  } while (Date.now() < until);
  throw new Error(`Guest wait timed out${error ? `: ${error.message}` : ""}`);
}

function registryVersion() {
  try {
    return execFileSync("reg.exe", ["query", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CortexOS", "/v", "DisplayVersion"], {encoding:"utf8", stdio:["ignore","pipe","ignore"]}).trim().split(/\s+/).at(-1);
  } catch { return null; }
}

async function install(exe, version) {
  await run(exe, ["/S"], {timeout:120_000, windowsHide:true});
  assert.equal(registryVersion(), version);
  assert.ok(existsSync(program) && existsSync(uninstaller));
}

async function withApp(action) {
  const child = spawn(program, [], { env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}` }, windowsHide:true, stdio:"ignore" });
  let socket;
  try {
    const target = await waitFor(async () => (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((item) => item.type === "page" && item.url.includes("tauri.localhost")));
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await once(socket, "open");
    let sequence = 0;
    async function evaluate(expression) {
      const id = ++sequence;
      const answer = new Promise((done, reject) => {
        const timer = setTimeout(() => {
          socket.removeEventListener("message", receive);
          reject(new Error("No response from the application window"));
        }, 5000);
        function receive(ev) {
          const reply = JSON.parse(ev.data);
          if (reply.id !== id) return;
          clearTimeout(timer);
          socket.removeEventListener("message", receive);
          if (reply.error || reply.result.exceptionDetails) reject(new Error("Installed application command failed"));
          else done(reply.result.result.value);
        }
        socket.addEventListener("message", receive);
      });
      socket.send(JSON.stringify({ id, method:"Runtime.evaluate", params:{ expression, returnByValue:true, awaitPromise:true } }));
      return answer;
    }
    const invoke = (name, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)}, ${JSON.stringify(args)})`);
    await waitFor(() => invoke("bootstrap"));
    const result = await action(invoke, evaluate);
    // Ask the real app window to close through its normal handler.
    await evaluate(`(() => { const b = document.querySelector('[aria-label="关闭窗口"], [title="关闭窗口"]'); if (b) b.click(); return !!b; })()`).catch(() => false);
    await waitFor(() => child.exitCode !== null, 12_000);
    assert.equal(child.exitCode, 0);
    return result;
  } finally {
    socket?.close();
    if (child.exitCode === null) child.kill();
  }
}

function openDatabase(path, action, readOnly = true) {
  const sqlite = new DatabaseSync(path, { readOnly });
  try { return action(sqlite); } finally { sqlite.close(); }
}

function verifyDatabase(path) {
  return openDatabase(path, (db) => {
    assert.equal(db.prepare("PRAGMA quick_check").get().quick_check, "ok");
    assert.equal(db.prepare("PRAGMA foreign_key_check").all().length, 0);
    return db.prepare("SELECT max(version) AS version FROM schema_migrations").get().version;
  });
}

const text = "H4 synthetic legacy knowledge text";
const workspaceName = "H4 synthetic isolated workspace";
const matchesWorkspace = (workspace) => openDatabase(dbPath, (db) => {
  assert.equal(db.prepare("SELECT name FROM workspaces WHERE id=?").get(workspace).name, workspaceName);
  const versions = db.prepare("SELECT v.content FROM knowledge_versions v JOIN knowledge_nodes n ON v.node_id=n.id WHERE n.workspace_id=? ORDER BY v.version").all(workspace);
  assert.deepEqual(versions.map((row) => row.content), [text, text + " 2"]);
});

function seedKnowledge(workspace) {
  openDatabase(dbPath, (db) => {
    const key = randomUUID(), now = new Date().toISOString();
    db.exec("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE");
    db.prepare("INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at) VALUES(?,?,2,2,2,?,?)").run(key, workspace, now, now);
    for (const version of [1, 2]) {
      db.prepare("INSERT INTO knowledge_versions(node_id,version,title,kind,content,author,change_reason,created_at) VALUES(?,?,?,'knowledge',?,'user','h4-test',?)").run(key,version,"H4 synthetic citation",version === 1 ? text : text + " 2",now);
      db.prepare("INSERT INTO knowledge_approval_events VALUES(?,?,?,'user',?)").run(randomUUID(),key,version,now);
    }
    db.exec("COMMIT");
  }, false);
}

async function createBenchmarkHistory(invoke, workspaceId) {
  const name = "h4-synthetic-model";
  const digest = createHash("sha256").update(name).digest("hex");
  const details = { format:"gguf", family:"fixture", parameter_size:"synthetic", quantization_level:"synthetic" };
  let modelRequests = 0;
  const server = createServer((request, response) => {
    const reply = (data) => response.writeHead(200, {"Content-Type":"application/json"}).end(JSON.stringify(data));
    switch (request.url) {
      case "/api/version": return reply({version:"synthetic-h4"});
      case "/api/tags": return reply({models:[{name,digest,size:100,details}]});
      case "/api/ps": return reply({models:[]});
      case "/api/show": return reply({details,capabilities:["completion"],model_info:{"general.architecture":"fixture","fixture.context_length":8192}});
      case "/api/generate": return reply({done:true,done_reason:"unload"});
      case "/api/chat": {
        modelRequests++;
        const frames = [{message:{role:"assistant",content:"95"},done:false}, {message:{role:"assistant",content:""},done:true,done_reason:"stop",prompt_eval_count:20,eval_count:1}];
        return response.writeHead(200, {"Content-Type":"application/x-ndjson"}).end(frames.map((frame) => JSON.stringify(frame)).join("\n") + "\n");
      }
      default: return response.writeHead(404).end();
    }
  });
  server.listen(11434, "127.0.0.1");
  await once(server, "listening");
  try {
    const datasets = await invoke("list_benchmark_datasets", {workspaceId});
    const dataset = datasets.find((item) => item.id === "cortex-core-60" && item.version === 2);
    const input = { datasetId:dataset.id, datasetVersion:2, datasetHash:dataset.contentHash, caseIds:["math-001"], repetitions:1,
      models:[{kind:"local",local:{model:name,expectedDigest:digest,requirements:["completion"],parameters:{numCtx:4096,numPredict:64,temperature:0,timeoutSeconds:10}}}],
      maxOutputTokens:64, price:null, pythonImage:null, releaseModels:[], localKeepAliveSeconds:0 };
    const batch = await invoke("plan_standard_benchmark", {workspaceId,input});
    await invoke("start_standard_benchmark", {workspaceId,batchId:batch.id,approvedPlanHash:batch.planHash});
    const result = await waitFor(async () => {
      const current = await invoke("get_standard_benchmark", {workspaceId,batchId:batch.id});
      return current.status === "completed" && current;
    });
    assert.equal(modelRequests, 1);
    assert.ok(result.items[0].runId);
    return {batchId:batch.id, runId:result.items[0].runId};
  } finally { server.close(); }
}

async function uninstallPreserving() {
  await run(uninstaller, ["/S"], { timeout:120_000,windowsHide:true });
  await waitFor(() => !existsSync(program) && !registryVersion());
  assert.ok(existsSync(dbPath));
}

async function uninstallRemovingGuestData() {
  const control = join(process.env.APPDATA, "cortexos-h4-unrelated.txt");
  writeFileSync(control, "leave this unrelated guest file intact");
  const child = spawn(uninstaller, [], { stdio: "ignore" });
  try {
    const selectDeletion = `
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
$root=[System.Windows.Automation.AutomationElement]::RootElement
$always=[System.Windows.Automation.Condition]::TrueCondition
$deadline=[DateTime]::UtcNow.AddSeconds(25)
do {
  foreach($window in $root.FindAll('Children',$always)) {
    if ($window.Current.Name -notmatch 'CortexOS') { continue }
    $items=$window.FindAll('Descendants',$always)
    $delete=$items | Where-Object { $_.Current.Name -in @('Delete the application data','删除应用程序数据') } | Select-Object -First 1
    $confirm=$items | Where-Object { $_.Current.Name -in @('Uninstall','卸载','&Uninstall') -and $_.Current.IsEnabled } | Select-Object -First 1
    if ($delete -and $confirm) {
      $toggle=$delete.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
      if ($toggle.Current.ToggleState -ne 'Off') { throw 'The deletion checkbox was selected by default' }
      $toggle.Toggle()
      $confirm.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
      exit 0
    }
  }
  Start-Sleep -Milliseconds 250
} while ([DateTime]::UtcNow -lt $deadline)
throw 'Did not find the uninstall data option'
`;
    await run("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", selectDeletion], { timeout:30_000, windowsHide:true });
    await waitFor(() => !existsSync(program) && !registryVersion() && !existsSync(data) && !existsSync(join(process.env.LOCALAPPDATA, "com.cortexos.desktop")));
    assert.ok(existsSync(control));
  } finally {
    if (child.exitCode === null) child.kill();
  }
}

try {
  if (mode === "upgrade") {
    await install(old, "0.2.0");
    const workspace = await withApp(async (invoke) => (await invoke("create_workspace", {name:workspaceName})).id);
    seedKnowledge(workspace);
    assert.equal(verifyDatabase(dbPath), 7);
    matchesWorkspace(workspace);
    record("published 0.2.0 installed and opened", { schema:7 });
    const originalProgram = digest(program);
    const blocker = spawn("powershell.exe", ["-NoProfile","-NonInteractive","-Command", `$f=[IO.File]::Open('${program}','Open','Read','Read'); [Console]::WriteLine('locked'); Start-Sleep 150; $f.Dispose()`], {windowsHide:true});
    try {
      const [message] = await once(blocker.stdout, "data");
      assert.match(String(message), /locked/);
      let failure;
      try { await run(candidate, ["/S"], {timeout:90_000,windowsHide:true}); } catch (error) { failure = error; }
      assert.ok(failure && Number.isInteger(failure.code) && failure.code !== 0, "The upgrade should report a nonzero installer exit");
      assert.equal(digest(program), originalProgram);
      assert.equal(verifyDatabase(dbPath), 7);
      matchesWorkspace(workspace);
      record("failed upgrade left the old binary and database readable", { exitCode:failure.code });
    } finally {
      blocker.kill();
      if (blocker.exitCode === null) await once(blocker, "exit");
    }
    await install(candidate, metadata.version);
    await withApp(async (invoke) => { const bootstrap = await invoke("bootstrap"); assert.ok(bootstrap.workspaces.some((item) => item.id === workspace)); });
    assert.equal(verifyDatabase(dbPath), 26);
    matchesWorkspace(workspace);
    const backup = readdirSync(data, {recursive:true}).find((name) => /before-migration-7-.*\.sqlite3$/.test(name));
    assert.ok(backup);
    assert.equal(verifyDatabase(join(data, backup)), 7);
    record("published release upgraded with migration backup and preserved knowledge versions", { previousSchema:7, schema:26 });
  } else {
    await install(candidate, metadata.version);
    const workspace = await withApp(async (invoke) => (await invoke("create_workspace", {name:workspaceName})).id);
    assert.equal(verifyDatabase(dbPath), 26);
    const connection = randomUUID(), secret = `synthetic-${randomUUID()}`;
    openDatabase(dbPath, (db) => db.prepare("INSERT INTO model_cloud_connections VALUES(?,?,?)").run(workspace,connection,JSON.stringify({id:connection,provider:"anthropic",label:"H4 synthetic connection",address:null,compatibleRerank:false,catalogState:"unconfigured",catalogCheckedAt:null,models:[],probes:[],limits:[]})), false);
    const history = await withApp(async (invoke) => {
      assert.equal((await invoke("update_channels")).version, metadata.version);
      await invoke("bind_runtime_cloud_key", {workspaceId:workspace,connectionId:connection,apiKey:secret});
      const list = await invoke("list_runtime_cloud_connections", {workspaceId:workspace});
      assert.ok(list.some((item) => item.credentialAvailable));
      return createBenchmarkHistory(invoke, workspace);
    });
    assert.equal(readFileSync(dbPath).includes(secret), false);
    assert.equal(openDatabase(dbPath, (db) => db.prepare("SELECT app_version FROM benchmark_runs WHERE id=?").get(history.runId).app_version), metadata.version);
    record("fresh candidate installation, first launch, restart and synthetic credential binding", { schema:26 });
    await uninstallPreserving();
    assert.equal(verifyDatabase(dbPath), 26);
    await install(candidate, metadata.version);
    await withApp(async (invoke) => {
      const list = await invoke("list_runtime_cloud_connections", {workspaceId:workspace});
      assert.ok(list.some((item) => item.credentialAvailable));
      assert.ok((await invoke("bootstrap")).workspaces.some((item) => item.id === workspace));
      const batch = await invoke("get_standard_benchmark", {workspaceId:workspace,batchId:history.batchId});
      assert.equal(batch.status, "completed");
      assert.equal(batch.items[0].runId, history.runId);
    });
    record("default uninstallation and reinstall preserved database, evaluation and credential", { schema:26 });
    await uninstallRemovingGuestData();
    record("explicit uninstall data deletion scoped to the disposable guest", { unrelatedFilePreserved:true });
  }
  report.finished = new Date().toISOString();
} catch (error) {
  report.failure = { name:error.name, message:error.message };
  process.exitCode = 1;
} finally {
  evidence();
}

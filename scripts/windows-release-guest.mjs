// Run only from an explicitly mapped Windows Sandbox test input directory.
import assert from "node:assert/strict";
import { execFile, execFileSync, spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { once } from "node:events";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmdirSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";

const input = "C:\\CortexOS-input";
const output = "C:\\CortexOS-output";
const mode = process.argv[2];
const resumeFresh = mode === "fresh" && process.argv[3] === "--resume";
const resumeUpgrade = mode === "upgrade" && process.argv[3] === "--resume";
const resume = resumeFresh || resumeUpgrade;
assert.equal(process.env.USERNAME, "WDAGUtilityAccount", "Requires a disposable Windows Sandbox account");
assert.ok(["fresh", "upgrade"].includes(mode));
const run = promisify(execFile);
const programDirectory = join(process.env.LOCALAPPDATA, "CortexOS");
const program = join(programDirectory, "cortexos-desktop.exe");
const uninstaller = join(programDirectory, "uninstall.exe");
const data = join(process.env.APPDATA, "com.cortexos.desktop");
const dbPath = join(data, "cortexos.sqlite3");
const reportFile = join(output, `${mode}.json`);
const report = resume ? JSON.parse(readFileSync(reportFile, "utf8")) : { mode, cases: [] };
const resumeRemoval = resumeFresh && report.cases.length === 3;
if (resume) {
  assert.ok(report.cases.length === 1 || resumeRemoval);
  assert.equal(report.cases[0].name, resumeFresh ? "fresh candidate installation and first launch" : "published 0.2.0 installed and opened");
  if (resumeRemoval) assert.equal(report.cases[2].name, "default uninstallation and reinstall preserved database, evaluation and credential");
  assert.ok(report.failure && report.finished);
  copyFileSync(reportFile, join(output, `${mode}-attempt-${randomUUID()}.json`));
  delete report.failure;
  delete report.finished;
  report.resumed = true;
}
const digest = (name) => createHash("sha256").update(readFileSync(name)).digest("hex");
const evidence = () => writeFileSync(reportFile, JSON.stringify(report, null, 2) + "\n");
const record = (name, detail) => { report.cases.push({ name, result: "passed", detail }); evidence(); };
const metadata = JSON.parse(readFileSync(join(input, "candidate.json"), "utf8").replace(/^\uFEFF/, ""));
assert.match(metadata.filename, /^CortexOS_\d+\.\d+\.\d+(?:-rc\.\d+)?_x64-setup\.exe$/);
const local = mkdtempSync(join(process.env.TEMP, "cortexos-h4-"));
function stage(filename, expected) {
  const exe = join(local, filename);
  copyFileSync(join(input, filename), exe);
  assert.equal(digest(exe), expected.toLowerCase());
  return exe;
}
const old = stage("CortexOS_0.2.0_x64-setup.exe", "b3a18e39b590c2615b024293e172e434949b263daf37cd7d122ff8c71a9ed1eb");
const candidate = stage(metadata.filename, metadata.sha256);
assert.equal(existsSync(dbPath), resume);
assert.equal(existsSync(program), resume);
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

function registryValue(key, value) {
  try {
    return execFileSync("reg.exe", ["query", key, "/v", value], {encoding:"utf8", stdio:["ignore","pipe","ignore"]}).trim().split(/\s+/).at(-1);
  } catch { return null; }
}

const registryVersion = () => registryValue("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CortexOS", "DisplayVersion");

function webViewVersion() {
  const id = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
  return [`HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\${id}`, `HKCU\\Software\\Microsoft\\EdgeUpdate\\Clients\\${id}`]
    .map((key) => registryValue(key,"pv")).find((value) => value && value !== "0.0.0.0") ?? null;
}

async function prepareWebView() {
  const before = webViewVersion();
  report.webView2 = { before, after: before };
  evidence();
  if (before) return;
  const checksum = join(input, "webview2.sha256");
  assert.ok(existsSync(checksum), "Evergreen WebView2 is missing, and no verified offline installer was supplied");
  const runtime = stage("MicrosoftEdgeWebView2RuntimeInstallerX64.exe", readFileSync(checksum, "ascii").trim());
  await run(runtime, ["/silent", "/install"], { timeout:180_000, windowsHide:true });
  report.webView2.after = await waitFor(webViewVersion);
  evidence();
}

async function install(exe, version) {
  await run(exe, ["/S"], {timeout:120_000, windowsHide:true});
  assert.equal(registryVersion(), version);
  assert.ok(existsSync(program) && existsSync(uninstaller));
}

async function withApp(action) {
  const child = spawn(program, [], { windowsHide:true, stdio:"ignore" });
  const ui = (operation, name, value = "") => guestUi(child.pid, operation, name, value);
  try {
    await ui("find", "^新建工作区$");
    const result = await action(ui);
    await ui("click", "^关闭窗口$");
    await waitFor(() => child.exitCode !== null, 12_000);
    assert.equal(child.exitCode, 0);
    return result;
  } finally {
    if (child.exitCode === null) child.kill();
  }
}

async function guestUi(pid, operation, name, value) {
  const literal = (text) => `'${text.replaceAll("'", "''")}'`;
  const script = `
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
$root=[System.Windows.Automation.AutomationElement]::RootElement
$scope=[System.Windows.Automation.TreeScope]::Descendants
$all=[System.Windows.Automation.Condition]::TrueCondition
$name=${literal(name)}
$action=${literal(operation)}
$content=${literal(value)}
$deadline=[DateTime]::UtcNow.AddSeconds(24)
$seen=@()
do {
  $handle=(Get-Process -Id ${pid} -ErrorAction Stop).MainWindowHandle
  if (!$handle) { Start-Sleep -Milliseconds 250; continue }
  $window=[System.Windows.Automation.AutomationElement]::FromHandle($handle)
  $items=@($window.FindAll($scope,$all))
  $seen=@($items | Where-Object {$_.Current.Name} | Select-Object -First 45 | ForEach-Object {$_.Current.Name})
  $found=@($items | Where-Object {$_.Current.Name -match $name})
  [array]::Reverse($found)
  foreach ($item in $found) {
    if ($action -eq 'find') { exit 0 }
    if (!$item.Current.IsEnabled) { continue }
    if ($action -eq 'enabled') { exit 0 }
    $pattern=$null
    if ($action -eq 'click' -and $item.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) {
      $pattern.Invoke(); exit 0
    }
    if ($action -in @('click','check') -and $item.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$pattern)) {
      if ($action -eq 'click' -or $pattern.Current.ToggleState -ne 'On') { $pattern.Toggle() }; exit 0
    }
    if ($action -eq 'set' -and $item.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) {
      $item.SetFocus()
      if ($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)) {
        try { $pattern.SetValue($content); exit 0 } catch [System.InvalidOperationException] {}
      }
      if ($content -notmatch '^[A-Za-z0-9 -]+$') { throw 'Unexpected guest keyboard content' }
      Add-Type -AssemblyName System.Windows.Forms
      [System.Windows.Forms.SendKeys]::SendWait('^a'+$content); exit 0
    }
  }
  Start-Sleep -Milliseconds 250
} while ([DateTime]::UtcNow -lt $deadline)
throw ('No guest UI element for ' + $action + ' ' + $name + '. Observed names: ' + ($seen -join ', '))
`;
  try {
    await run("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], { timeout:35_000, windowsHide:true });
  } catch (error) {
    throw new Error(`Guest UI ${operation} ${name} failed: ${error.stderr ?? error.message}`, { cause:error });
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
const savedWorkspace = () => openDatabase(dbPath, (db) => db.prepare("SELECT id FROM workspaces WHERE name=?").get(workspaceName)?.id);
async function createWorkspace(ui) {
  await ui("click", "^新建工作区$");
  await ui("set", "^工作区名称$", workspaceName);
  await ui("click", "^创建工作区$");
  return waitFor(savedWorkspace);
}
const matchesWorkspace = (workspace) => openDatabase(dbPath, (db) => {
  assert.equal(db.prepare("SELECT name FROM workspaces WHERE id=?").get(workspace).name, workspaceName);
  const node = db.prepare("SELECT latest_version, effective_version FROM knowledge_nodes WHERE workspace_id=?").get(workspace);
  assert.equal(node.latest_version, 2);
  assert.equal(node.effective_version, 2);
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

async function attemptBlockedUpgrade() {
  const savedProgram = join(local, "legacy-desktop.exe");
  const marker = join(program, "h4-obstruction.txt");
  renameSync(program, savedProgram);
  let failure;
  try {
    mkdirSync(program);
    writeFileSync(marker, "guest fixture occupies the executable path");
    try { await run(candidate, ["/S"], { timeout:90_000, windowsHide:true }); } catch (error) { failure = error; }
    report.destinationTest = { exitCode:failure?.code ?? 0, markerPreserved:existsSync(marker) };
    evidence();
    assert.equal(readFileSync(marker, "utf8"), "guest fixture occupies the executable path");
    assert.ok(failure && Number.isInteger(failure.code) && failure.code !== 0, "The upgrade to the obstructed executable path should report a nonzero installer exit");
    return failure.code;
  } finally {
    if (existsSync(program) && statSync(program).isDirectory()) {
      if (existsSync(marker)) unlinkSync(marker);
      rmdirSync(program);
    }
    if (!existsSync(program)) renameSync(savedProgram, program);
  }
}

function benchmarkHistory(workspaceId) {
  return openDatabase(dbPath, (db) => db.prepare(`
    SELECT b.id AS batchId, i.run_id AS runId, r.app_version AS appVersion
    FROM benchmark_batches b JOIN benchmark_batch_items i ON i.batch_id=b.id
    JOIN benchmark_runs r ON r.id=i.run_id
    WHERE b.workspace_id=? AND b.status='completed' AND i.case_id='math-001'
  `).get(workspaceId));
}

async function createBenchmarkHistory(ui, workspaceId) {
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
    await ui("click", "^评测$");
    await ui("check", `^${name}$`);
    await ui("click", "^清空选择$");
    await ui("set", "^查找题目$", "math-001");
    await ui("check", "^math-001");
    await ui("click", "^预览标准评测计划$");
    await ui("check", "^我已检查题库、模型和发送范围");
    await ui("click", "^确认并运行未发送题目$");
    const result = await waitFor(() => benchmarkHistory(workspaceId));
    assert.equal(modelRequests, 1);
    assert.equal(result.appVersion, metadata.version);
    return result;
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
  const selectDeletion = String.raw`
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class GuestUninstaller {
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint message, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
$root=[System.Windows.Automation.AutomationElement]::RootElement
$always=[System.Windows.Automation.Condition]::TrueCondition
$deadline=[DateTime]::UtcNow.AddSeconds(25)
$titles='^Installer Language$|^CortexOS\s+(?:Uninstall|卸载)\s*$'
if (!($root.FindAll('Children',$always) | Where-Object { $_.Current.Name -match $titles })) {
  Start-Process -WindowStyle Normal -FilePath (Join-Path $env:LOCALAPPDATA 'CortexOS\uninstall.exe')
}
function Press-Button($window,$button) {
  if (!$button.Current.NativeWindowHandle) { throw 'The uninstaller control has no native handle' }
  [void][GuestUninstaller]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle)
  [void][GuestUninstaller]::SendMessage([IntPtr]$button.Current.NativeWindowHandle,0x00F5,[IntPtr]::Zero,[IntPtr]::Zero)
}
do {
  foreach($window in $root.FindAll('Children',$always)) {
    if ($window.Current.Name -notmatch $titles) { continue }
    $items=$window.FindAll('Descendants',$always)
    if ($window.Current.Name -eq 'Installer Language') {
      $next=$items | Where-Object { $_.Current.Name -eq 'OK' } | Select-Object -First 1
      if ($next) { Press-Button $window $next }; continue
    }
    $delete=$items | Where-Object { $_.Current.Name -in @('Delete the application data','删除应用程序数据') } | Select-Object -First 1
    $confirm=$items | Where-Object { $_.Current.Name -match '^(?:&?Uninstall|卸载)(?:\s*\(&?[A-Z]\))?$' -and $_.Current.IsEnabled } | Select-Object -First 1
    if ($delete -and $confirm) {
      $check=[IntPtr]$delete.Current.NativeWindowHandle
      if ($check -eq [IntPtr]::Zero -or [GuestUninstaller]::SendMessage($check,0x00F0,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -ne 0) { throw 'The deletion checkbox was selected by default or unavailable' }
      Press-Button $window $delete
      if ([GuestUninstaller]::SendMessage($check,0x00F0,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -ne 1) { throw 'The deletion checkbox did not become selected' }
      Press-Button $window $confirm
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
}

async function verifyExplicitUninstall() {
  await uninstallRemovingGuestData();
  record("explicit uninstall data deletion scoped to the disposable guest", { unrelatedFilePreserved:true });
}

try {
  if (!resume) await prepareWebView();
  if (resumeRemoval) {
    assert.equal(registryVersion(), metadata.version);
    assert.equal(verifyDatabase(dbPath), 26);
    await verifyExplicitUninstall();
  } else if (mode === "upgrade") {
    if (resumeUpgrade && registryVersion() && registryVersion() !== "0.2.0") await uninstallPreserving();
    if (registryVersion() !== "0.2.0") await install(old, "0.2.0");
    const workspace = resumeUpgrade ? savedWorkspace() : await withApp(createWorkspace);
    assert.ok(workspace);
    if (!resumeUpgrade) seedKnowledge(workspace);
    assert.equal(verifyDatabase(dbPath), 7);
    matchesWorkspace(workspace);
    if (!resumeUpgrade) record("published 0.2.0 installed and opened", { schema:7 });
    const originalProgram = digest(program);
    const failureExit = await attemptBlockedUpgrade();
    assert.equal(registryVersion(), "0.2.0");
    assert.equal(digest(program), originalProgram);
    assert.equal(verifyDatabase(dbPath), 7);
    matchesWorkspace(workspace);
    record("failed upgrade left the old binary and database readable", { exitCode:failureExit, fault:"executable-path-is-directory" });
    await install(candidate, metadata.version);
    await withApp((ui) => ui("find", `^${workspaceName}$`));
    assert.equal(verifyDatabase(dbPath), 26);
    matchesWorkspace(workspace);
    const backup = readdirSync(data, {recursive:true}).find((name) => /before-migration-7-.*\.sqlite3$/.test(name));
    assert.ok(backup);
    assert.equal(verifyDatabase(join(data, backup)), 7);
    record("published release upgraded with migration backup and preserved knowledge versions", { previousSchema:7, schema:26 });
  } else {
    if (!resumeFresh) await install(candidate, metadata.version);
    assert.equal(registryVersion(), metadata.version);
    const workspace = resumeFresh ? savedWorkspace() : await withApp(createWorkspace);
    assert.ok(workspace);
    assert.equal(verifyDatabase(dbPath), 26);
    if (!resumeFresh) record("fresh candidate installation and first launch", { schema:26 });
    const secret = `synthetic-${randomUUID()}`;
    let connection = openDatabase(dbPath, (db) => db.prepare("SELECT id FROM model_cloud_connections WHERE workspace_id=?").get(workspace)?.id);
    if (!connection) {
      connection = randomUUID();
      openDatabase(dbPath, (db) => db.prepare("INSERT INTO model_cloud_connections VALUES(?,?,?)").run(workspace,connection,JSON.stringify({id:connection,provider:"anthropic",label:"H4 synthetic connection",address:null,compatibleRerank:false,catalogState:"unconfigured",catalogCheckedAt:null,models:[],probes:[],limits:[]})), false);
    }
    const history = await withApp(async (ui) => {
      await ui("click", "^设置$");
      await ui("click", "^关于$");
      await ui("find", "^应用更新$");
      await ui("click", "^云端模型$");
      await ui("set", "^API 密钥$", secret);
      await ui("click", "^绑定密钥$");
      await ui("enabled", "^刷新模型列表$");
      return createBenchmarkHistory(ui, workspace);
    });
    assert.equal(readFileSync(dbPath).includes(secret), false);
    assert.ok(history.runId);
    record("restart, synthetic credential and standard evaluation using a loopback fixture", { schema:26, evaluation:history });
    await uninstallPreserving();
    assert.equal(verifyDatabase(dbPath), 26);
    await install(candidate, metadata.version);
    await withApp(async (ui) => {
      await ui("find", `^${workspaceName}$`);
      await ui("click", "^设置$");
      await ui("click", "^云端模型$");
      await ui("enabled", "^刷新模型列表$");
    });
    assert.deepEqual(benchmarkHistory(workspace), history);
    record("default uninstallation and reinstall preserved database, evaluation and credential", { schema:26 });
    await verifyExplicitUninstall();
  }
  report.finished = new Date().toISOString();
} catch (error) {
  report.failure = { name:error.name, message:error.message, code:error.code, signal:error.signal, killed:error.killed, stdout:error.stdout, stderr:error.stderr };
  report.finished = new Date().toISOString();
  process.exitCode = 1;
} finally {
  evidence();
}

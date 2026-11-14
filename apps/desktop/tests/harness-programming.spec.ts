import { expect,test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createWorkspace,launchNative,nativeInvoke } from './native';
import type { ProgrammingPayload } from '../src/benchmark/harness/programmingTypes';
import type { Snapshot } from '../src/benchmark/harness/types';

test('programming drafts keep immutable sources, preserve edits and refuse unverified publication',async({},info)=>{
 test.setTimeout(180000);
 await mkdir(info.outputPath('fixtures'),{recursive:true});
 const doc=info.outputPath('fixtures/development.md'),zip=info.outputPath('fixtures/project.zip');
 await writeFile(doc,'Use Python pure functions.\nKeep source snapshots immutable.\n');
 await promisify(execFile)('py',['-3','-X','utf8','-c','import sys,zipfile\nwith zipfile.ZipFile(sys.argv[1],"w") as z:\n z.writestr("src/value.py","def adjust(n):\\n    return n\\n")',zip],{windowsHide:true});
 const app=await launchNative(info,{sourcePath:doc});
 try{
  const {page}=app;page.setDefaultTimeout(15000);
  await createWorkspace(page,'D2b 编程草案验收');
  const workspaceId=await page.getByLabel('当前工作区',{exact:true}).inputValue();
  const snapshot=await nativeInvoke<Snapshot>(page,'import_harness_document',{workspaceId});
  await nativeInvoke(page,'save_harness_fit',{workspaceId,input:{snapshotIds:[snapshot.id],requirements:[],weights:{compatibility:3,automation:3,governance:3,context:3},expectedVersion:0,note:''}});
  await page.getByRole('navigation',{name:'主导航',exact:true}).getByRole('link',{name:'评测',exact:true}).click();
  await page.getByRole('link',{name:'项目适配',exact:true}).click();
  await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();
  await page.getByText('编程任务草案',{exact:true}).click();
  const form=page.locator('.programming-drafts');
  await form.getByRole('button',{name:'选择项目ZIP',exact:true}).click();
  await promisify(execFile)('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','选择项目ZIP','-OutputPath',zip],{windowsHide:true,timeout:25000});
  await expect(page.getByRole('combobox',{name:'编程任务目标文件',exact:true})).toHaveValue('src/value.py');
  await page.getByRole('spinbutton',{name:'原函数结束行',exact:true}).fill('2');
  await page.getByRole('textbox',{name:'编程任务依据',exact:true}).fill('受控功能需求：负数归零，非负数保持。');
  await page.getByRole('textbox',{name:'编程任务预期行为',exact:true}).fill('输入整数n，负数返回0，非负数返回n。');
  await page.getByRole('textbox',{name:'编程任务函数名',exact:true}).fill('adjust');
  await page.getByRole('textbox',{name:'参数 1 名称',exact:true}).fill('n');
  for(let i=0;i<7;i++){
   await form.getByRole('button',{name:'添加用例',exact:true}).click();
   await page.getByRole('textbox',{name:`用例 ${i+1} 说明`,exact:true}).fill(`输入 ${i-3}`);
   await page.getByRole('textbox',{name:`用例 ${i+1} 参数`,exact:true}).fill(JSON.stringify([i-3]));
   await page.getByRole('textbox',{name:`用例 ${i+1} 输出`,exact:true}).fill(JSON.stringify(Math.max(0,i-3)));
  }
  await page.getByRole('combobox',{name:'用例 3 用途',exact:true}).selectOption('target');
  await page.getByRole('combobox',{name:'用例 2 用途',exact:true}).selectOption('target');
  await page.getByRole('combobox',{name:'用例 4 用途',exact:true}).selectOption('boundary');
  await page.getByRole('combobox',{name:'用例 5 用途',exact:true}).selectOption('regression');
  await page.getByRole('textbox',{name:'参考实现代码',exact:true}).fill('def adjust(n):\n    return max(n, 0)');
  await page.getByRole('textbox',{name:'错误对照代码',exact:true}).fill('def adjust(n):\n    return 0');
  await page.getByRole('textbox',{name:'用例 1 输出',exact:true}).fill('true');
  await form.getByRole('button',{name:'保存编程草案',exact:true}).click();
  await expect(form.getByRole('alert')).toContainText('类型');
  await expect(page.getByRole('textbox',{name:'参考实现代码',exact:true})).toHaveValue('def adjust(n):\n    return max(n, 0)');
  await page.getByRole('textbox',{name:'用例 1 输出',exact:true}).fill('0');
  await form.getByRole('button',{name:'保存编程草案',exact:true}).click();
  await expect(form.getByRole('button',{name:'保存编程草案',exact:true})).toBeDisabled();
  const initial=await nativeInvoke<ProgrammingPayload>(page,'get_harness_programming_drafts',{workspaceId,jobId:null}),job=initial.selected!,v1=job.revisions[0];
  expect(v1.body.source.excerpt).toBe('def adjust(n):\n    return n');
  expect(v1.body.validation.publishable).toBe(false);
  expect(v1.body.validation.checks.every(c=>c.actual==='Not Run')).toBe(true);
  await expect(page.getByRole('combobox',{name:'编程任务目标文件',exact:true})).toBeDisabled();
  expect(await nativeInvoke(page,'publish_harness_tasks',{workspaceId,jobId:job.id,selection:[{id:job.id,revision:1,approved:true}],reviewer:'不可绕过',expectedVersion:0}).then(()=>false,()=>true)).toBe(true);
  await page.getByRole('textbox',{name:'编程任务依据',exact:true}).fill('跨步骤保留的未保存修改');
  await page.getByRole('button',{name:'3. 适配结果',exact:true}).click();
  expect(await page.evaluate(()=>{const e=new Event('beforeunload',{cancelable:true});window.dispatchEvent(e);return e.defaultPrevented;})).toBe(true);
  await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();
  await page.getByText('编程任务草案',{exact:true}).click();
  await expect(page.getByRole('textbox',{name:'编程任务依据',exact:true})).toHaveValue('跨步骤保留的未保存修改');
  await form.getByRole('button',{name:'另建草案',exact:true}).click();
  await expect(page.getByRole('dialog',{name:'放弃未保存的修改',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'继续编辑',exact:true}).click();
  await form.getByRole('button',{name:'保存编程草案',exact:true}).click();
  await expect(page.getByRole('combobox',{name:'编程草案修订记录',exact:true})).toHaveValue('2');
  await page.getByRole('combobox',{name:'编程草案修订记录',exact:true}).selectOption('1');
  await expect(page.getByRole('textbox',{name:'编程任务依据',exact:true})).toHaveValue(v1.body.spec.rationale);
  await expect(page.getByRole('textbox',{name:'编程任务预期行为',exact:true})).toBeDisabled();
  await page.getByRole('combobox',{name:'编程草案修订记录',exact:true}).selectOption('2');
  await page.getByText('保存依据与待验证项目',{exact:true}).click();
  await form.getByRole('button',{name:'查看保存时的来源',exact:true}).click();
  await expect(page.getByRole('dialog',{name:/src\/value.py/})).toBeVisible();
  await expect(page.locator('.fit-source-dialog [data-active=true]')).toHaveCount(2);
  await page.getByRole('button',{name:'关闭任务原文',exact:true}).click();
  for(const theme of ['light','dark']){
   await page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
   await expect(page.locator('html')).toHaveAttribute('data-color-scheme',theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});
    await page.locator('.fit-scroll').evaluate(e=>{const r=e.querySelector('.programming-drafts')!.getBoundingClientRect();e.scrollTop+=r.top-e.getBoundingClientRect().top-12;});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    expect(await page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);
    await page.screenshot({path:info.outputPath(`drafts-${theme}-${width}.png`)});
   }
  }
  await page.reload();
  await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();
  await page.getByText('编程任务草案',{exact:true}).click();
  await expect(page.getByRole('textbox',{name:'编程任务依据',exact:true})).toHaveValue('跨步骤保留的未保存修改');
  const reloaded=await nativeInvoke<ProgrammingPayload>(page,'get_harness_programming_drafts',{workspaceId,jobId:job.id});
  expect(reloaded.selected!.revisions[1]).toEqual(v1);
  await writeFile(info.outputPath('programming.json'),JSON.stringify({workspaceId,jobId:job.id,revisions:reloaded.selected!.revisions.length,containerValidation:'Not Run',noModelCalls:true,noCodeExecution:true},null,2));
  expect(app.consoleErrors).toEqual([]);
 }catch(error){await app.page.screenshot({path:info.outputPath('failure.png')});await writeFile(info.outputPath('failure-dom.json'),JSON.stringify(await app.page.evaluate(()=>({text:document.body.innerText,details:[...document.querySelectorAll('details')].map(e=>({title:e.querySelector('summary')?.textContent,open:e.open})),select:document.querySelector('[aria-label="编程草案修订记录"]')?.outerHTML})),null,2));throw error;}finally{await app.stop();}
});

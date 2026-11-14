import { expect,test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve } from 'node:path';
import { writeFile } from 'node:fs/promises';
import { createWorkspace,launchNative,nativeInvoke } from './native';
import type { Body,Payload } from '../src/benchmark/harness/types';

test('local document fit imports actual development spec and retains reviewed evidence versions',async({},info)=>{
 test.setTimeout(180000);
 const app=await launchNative(info);
 const fileDialog=async(cancel:boolean)=>promisify(execFile)('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','选择项目规范文档',...(cancel?['-Cancel']:['-OutputPath',resolve('../../development.md')])],{windowsHide:true,timeout:25000});
 try{
  const {page}=app;page.setDefaultTimeout(12000);
  await createWorkspace(page,'Harness Fit 真实文档验收');
  const workspaceId=await page.getByLabel('当前工作区',{exact:true}).inputValue();
  expect(await nativeInvoke(page,'personalized_access').then(()=>false,()=>true)).toBe(true);
  await page.getByRole('navigation',{name:'主导航',exact:true}).getByRole('link',{name:'评测',exact:true}).click();
  await page.getByRole('link',{name:'项目适配',exact:true}).click();
  await expect(page.getByRole('navigation',{name:'项目适配步骤',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'项目适配',exact:true})).toHaveCount(0);
  await expect(page.locator('.fit-catalog-item .fit-status').filter({hasText:'安装版本未知'})).toHaveCount(5);
  for(const [theme,width,height] of [['light',1366,768],['dark',1600,900]] as const){
   await page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);await page.setViewportSize({width,height});
   await page.screenshot({path:info.outputPath(`catalog-status-${theme}.png`)});
  }
  await page.getByRole('combobox',{name:'外观',exact:true}).selectOption('light');await page.setViewportSize({width:1440,height:900});
  await page.getByRole('button',{name:'导入规范文档',exact:true}).click();await fileDialog(true);
  await expect(page.getByRole('button',{name:'导入规范文档',exact:true})).toBeEnabled();
  expect((await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:null})).snapshots).toHaveLength(0);
  await page.getByRole('button',{name:'导入规范文档',exact:true}).click();await fileDialog(false);
  await expect(page.locator('.fit-selected-docs')).toContainText('development.md');
  await page.getByRole('button',{name:'分析所选资料',exact:true}).click();
  await expect(page.getByRole('heading',{name:'确认项目要求',exact:true})).toBeVisible();
  const data=await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:null});
  expect(data.categories).toHaveLength(12);expect(data.catalog.entries).toHaveLength(5);
  const initial=await nativeInvoke<Body>(page,'analyze_harness_fit',{workspaceId,input:{snapshotIds:[data.snapshots[0].id],requirements:[],weights:{compatibility:3,automation:3,governance:3,context:3},expectedVersion:0,note:''}});
  const rule=initial.requirements.find(r=>r.criterion==='human_approval')!;expect(rule).toBeTruthy();
  const row=page.locator(`[data-requirement-id="${rule.id}"]`);
  await page.locator('.fit-category').filter({has:row}).locator('summary').first().click();
  await row.getByRole('combobox').first().selectOption('confirmed');await row.getByRole('checkbox',{name:'必须满足',exact:true}).check();
  await row.getByText(/查看依据 ·/).click();await row.getByRole('button',{name:'打开原文位置',exact:true}).click();
  const source=page.getByRole('dialog',{name:/development.md/});await expect(source).toBeVisible();
  await expect(source.locator('[data-active=true]')).toContainText(rule.evidence!.excerpt);
  await source.getByRole('button',{name:'关闭项目原文',exact:true}).click();
  await page.getByRole('button',{name:'查看适配结果',exact:true}).click();
  await expect(page.getByRole('heading',{name:'项目与工具的静态适配',exact:true})).toBeVisible();
  await expect(page.getByRole('row').filter({has:page.getByRole('rowheader',{name:'OpenHands',exact:true})})).toContainText('不符合必须条件');
  await page.getByRole('button',{name:'保存新版本',exact:true}).click();
  await expect.poll(async()=>(await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:null})).selected?.version).toBe(1);
  const v1=(await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:1})).selected!;
  await page.getByLabel('权限与审批权重',{exact:true}).fill('5');
  await page.getByRole('button',{name:'更新分析',exact:true}).click();
  await page.getByRole('button',{name:'保存新版本',exact:true}).click();
  await expect.poll(async()=>(await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:null})).selected?.version).toBe(2);
  await page.getByRole('combobox',{name:'分析历史版本',exact:true}).selectOption('1');
  await expect(page.getByText(/正在查看历史版本/)).toBeVisible();await expect(page.getByRole('button',{name:'保存新版本',exact:true})).toBeDisabled();
  expect((await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:1})).selected).toEqual(v1);
  await page.getByRole('combobox',{name:'分析历史版本',exact:true}).selectOption('');
  await page.reload();await expect(page.getByRole('button',{name:'保存新版本',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'3. 适配结果',exact:true}).click();
  await expect(page.locator('.fit-comparison')).toBeVisible();
  expect(await page.evaluate(()=>{const event=new Event('beforeunload',{cancelable:true});window.dispatchEvent(event);return event.defaultPrevented;})).toBe(false);
  for(const theme of ['light','dark']){
   await page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});await expect(page.locator('.fit-comparison')).toBeVisible();await page.locator('.fit-comparison').scrollIntoViewIfNeeded();
    expect(await page.locator('#main-content').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);
    expect(await page.getByRole('button',{name:'保存新版本',exact:true}).evaluate(e=>{const r=e.getBoundingClientRect();return e.contains(document.elementFromPoint(r.left+r.width/2,r.top+r.height/2));})).toBe(true);
    await page.screenshot({path:info.outputPath(`fit-${theme}-${width}.png`)});
   }
  }
  const final=await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId,version:null});
  await writeFile(info.outputPath('analysis.json'),JSON.stringify(final,null,2));
  expect(final.selected!.body.requirements.find(r=>r.id===rule.id)?.hard).toBe(true);
  expect(final.selected!.body.report.results.every(r=>r.verifiedCount===0)).toBe(true);
  expect(await nativeInvoke(page,'list_conversations',{workspaceId})).toEqual([]);
  expect(await nativeInvoke(page,'list_standard_benchmarks',{workspaceId})).toEqual([]);
  const other=await nativeInvoke<{id:string}>(page,'create_workspace',{name:'隔离工作区'});
  expect((await nativeInvoke<Payload>(page,'get_harness_fit',{workspaceId:other.id,version:null})).selected).toBeNull();
  expect(app.consoleErrors).toEqual([]);
 }finally{await app.stop();}
});

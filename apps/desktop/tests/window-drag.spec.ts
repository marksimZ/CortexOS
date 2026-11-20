import { expectPicker } from "./select-picker";
import { expect,test } from "@playwright/test";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { launchNative,createWorkspace,nativeInvoke } from "./native";
const exec=promisify(execFile);
async function nativeWindow(pid:number,action:"drag"|"inspect"|"restore",point?:{x:number;y:number}){
 const {stdout}=await exec("powershell.exe",["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File",resolve("tests/window-drag.ps1"),"-AppProcessId",String(pid),"-Action",action,...(point?["-ClientX",String(Math.round(point.x)),"-ClientY",String(Math.round(point.y))]:[])],{windowsHide:true,timeout:15000});
 return JSON.parse(stdout.trim()) as {DeltaX:number;DeltaY:number;WidthChanged:number;HeightChanged:number;Minimized?:boolean};
}
test("native page titles and empty title bars move the real window without stealing controls",async({},info)=>{
 test.setTimeout(120000);
 const app=await launchNative(info);
 try{
  const {page}=app;await createWorkspace(page,"窗口拖动专项");
  const measurements=[];
  for(const [label,selector] of [["评测",".topbar-title strong"],["对话",".topbar-title strong"],["知识",".topbar-title strong"],["图谱",".topbar-title strong"],["设置",".topbar-title strong"]]){
   await page.getByRole("navigation",{name:"主导航",exact:true}).getByRole("link",{name:label,exact:true}).click();
   const header=page.locator(".prototype-topbar"),blank=page.locator(".window-drag-region");
   const headerBounds=await header.boundingBox(),contentBounds=await page.locator("#main-content").boundingBox();
   expect(headerBounds!.height).toBeGreaterThanOrEqual(48);
   expect(contentBounds!.y).toBeGreaterThanOrEqual(headerBounds!.y+headerBounds!.height);
   const blankBounds=await blank.boundingBox();expect(blankBounds!.width).toBeGreaterThan(32);
   const blankPoint=await blank.evaluate(e=>{const r=e.getBoundingClientRect();return{x:(r.left+r.width/2)*devicePixelRatio,y:(r.top+r.height/2)*devicePixelRatio}});
   const blankMoved=await nativeWindow(app.processId!,"drag",blankPoint);
   expect(blankMoved.DeltaX,`${label} blank titlebar should move the actual window`).toBeGreaterThan(60);
   expect(blankMoved.DeltaY).toBeGreaterThan(10);
   measurements.push({page:`${label}空白标题栏`,...blankMoved});
   const target=page.locator(selector).first();await expect(target).toBeVisible();
   const point=await target.evaluate(e=>{const r=e.getBoundingClientRect();return{x:(r.left+r.width/2)*devicePixelRatio,y:(r.top+r.height/2)*devicePixelRatio}});
   const moved=await nativeWindow(app.processId!,"drag",point);measurements.push({page:label,...moved});
   expect(moved.DeltaX,`${label} title should move the actual native window`).toBeGreaterThan(60);expect(moved.DeltaY).toBeGreaterThan(10);expect(moved.WidthChanged).toBe(0);expect(moved.HeightChanged).toBe(0);
   await page.screenshot({path:info.outputPath(`titlebar-${label}.png`)});
  }
  await page.getByRole("navigation",{name:"主导航",exact:true}).getByRole("link",{name:"对话",exact:true}).click();
  const blank=page.locator(".window-drag-region");
  const point=await blank.evaluate(e=>{const r=e.getBoundingClientRect();return{x:(r.left+r.width/2)*devicePixelRatio,y:(r.top+r.height/2)*devicePixelRatio}});
  const moved=await nativeWindow(app.processId!,"drag",point);expect(moved.DeltaX).toBeGreaterThan(60);measurements.push({page:"对话空白标题栏",...moved});
  const title=page.locator(".topbar-title strong");
  await title.dblclick();await expect.poll(()=>nativeInvoke<boolean>(page,"plugin:window|is_maximized",{label:"main"})).toBe(true);
  await page.getByRole("button",{name:"还原窗口",exact:true}).click();await expect.poll(()=>nativeInvoke<boolean>(page,"plugin:window|is_maximized",{label:"main"})).toBe(false);
  await page.getByRole("combobox",{name:"外观",exact:true}).click();await expectPicker(page.getByRole("combobox",{name:"外观",exact:true}), true);await page.keyboard.press("Escape");
  await page.getByRole("button",{name:"最小化窗口",exact:true}).click();await expect.poll(async()=>(await nativeWindow(app.processId!,"inspect")).Minimized).toBe(true);await nativeWindow(app.processId!,"restore");
  await page.screenshot({path:info.outputPath("window-titlebar.png")});
  await info.attach("actual-window-movement",{body:JSON.stringify(measurements,null,2),contentType:"application/json"});
  expect(app.consoleErrors).toEqual([]);
  const closed=page.waitForEvent("close");await page.getByRole("button",{name:"关闭窗口",exact:true}).click().catch(e=>{if(!page.isClosed())throw e;});await closed;
 }finally{await app.stop();}
});


test("native dialogs reserve the titlebar and keep window operations available",async({},info)=>{
 test.setTimeout(60000);
 const app=await launchNative(info);
 try {
  const {page}=app;await createWorkspace(page,"弹窗与窗口操作专项");
  await page.getByRole("button",{name:"新建工作区",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"创建工作区",exact:true});await expect(dialog).toBeVisible();
  expect(await page.locator("#main-content").evaluate(e=>(e as HTMLElement).inert)).toBe(true);
  expect(await page.locator(".prototype-topbar").evaluate(e=>!!e.closest("[inert]"))).toBe(false);
  const assertHeader=async()=>{
   const bounds=await dialog.boundingBox(),header=await page.locator(".prototype-topbar").boundingBox();
   expect(bounds!.y).toBeGreaterThanOrEqual(header!.y+header!.height+8);
   for(const name of ["最小化窗口","最大化窗口","关闭窗口"]){
    const button=page.getByRole("button",{name,exact:true});await expect(button).toBeVisible();
    expect(await button.evaluate(e=>{const r=e.getBoundingClientRect();return e.contains(document.elementFromPoint(r.left+r.width/2,r.top+r.height/2));})).toBe(true);
   }
  };
  for(const theme of ["light","dark"]){
   await page.getByRole("combobox",{name:"外观",exact:true}).selectOption(theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});await assertHeader();
    const rect=await dialog.boundingBox();expect(rect!.y+rect!.height).toBeLessThanOrEqual(height-16);
    await page.screenshot({path:info.outputPath(`dialog-titlebar-${theme}-${width}.png`)});
   }
  }
  // A modal must not block actual window API state changes.
  await page.getByRole("button",{name:"最大化窗口",exact:true}).click();await expect.poll(()=>nativeInvoke<boolean>(page,"plugin:window|is_maximized",{label:"main"})).toBe(true);
  await page.getByRole("button",{name:"还原窗口",exact:true}).click();await expect.poll(()=>nativeInvoke<boolean>(page,"plugin:window|is_maximized",{label:"main"})).toBe(false);
  await page.getByRole("button",{name:"最小化窗口",exact:true}).click();await expect.poll(async()=>(await nativeWindow(app.processId!,"inspect")).Minimized).toBe(true);await nativeWindow(app.processId!,"restore");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button",{name:"关闭弹窗",exact:true}).click();await expect(dialog).not.toBeVisible();
  expect(await page.locator("#main-content").evaluate(e=>(e as HTMLElement).inert)).toBe(false);
  for(const label of ["知识","图谱","评测","设置"]){
   await page.getByRole("navigation",{name:"主导航",exact:true}).getByRole("link",{name:label,exact:true}).click();
   await expect(page.locator('.topbar-title strong')).toHaveText(label);
   await expect(page.locator('#page-title')).toHaveClass(/sr-only/);
   if(label==='图谱'){
    const select=page.locator('#graph-source');
    const style=await select.evaluate(e=>{const s=getComputedStyle(e);return{left:parseFloat(s.paddingLeft),right:parseFloat(s.paddingRight),height:e.getBoundingClientRect().height};});
    expect(style.left).toBeGreaterThanOrEqual(12);expect(style.right).toBeGreaterThanOrEqual(30);expect(style.height).toBe(34);
    await page.locator('.segmented-control label').filter({hasText:'执行过程'}).click();
    await expect(page.getByRole('radio',{name:'执行过程',exact:true})).toBeChecked();
    await page.locator('.segmented-control label').filter({hasText:'知识关系'}).click();
    await expect(page.getByRole('radio',{name:'知识关系',exact:true})).toBeChecked();
   }
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});
    await page.screenshot({path:info.outputPath(`page-${label}-${width}.png`)});
   }
   const header=await page.locator(".prototype-topbar").boundingBox(),content=await page.locator("#main-content").boundingBox();
   expect(header!.height).toBeGreaterThanOrEqual(48);expect(content!.y).toBeGreaterThanOrEqual(header!.y+header!.height);
   expect((await page.locator(".window-drag-region").boundingBox())!.width).toBeGreaterThan(32);
   await page.getByRole("button",{name:"新建工作区",exact:true}).click();await expect(dialog).toBeVisible();await assertHeader();
   await page.keyboard.press("Escape");await expect(dialog).not.toBeVisible();
   await expect(page.getByRole("button",{name:"新建工作区",exact:true})).toBeFocused();
  }
  await page.getByRole("button",{name:"新建工作区",exact:true}).click();await expect(dialog).toBeVisible();
  for(let i=0;i<12;i++){
   await page.keyboard.press("Tab");expect(await page.evaluate(()=>!!document.activeElement?.closest("dialog[open],.prototype-topbar"))).toBe(true);
  }
  expect(app.consoleErrors).toEqual([]);
  const closed=page.waitForEvent("close");await page.getByRole("button",{name:"关闭窗口",exact:true}).click().catch(e=>{if(!page.isClosed())throw e;});await closed;
 } finally {await app.stop();}
});

import { notify } from '../../components/Toast';
import { FitStatus } from './FitStatus';
import type { Body,Fact,Requirement,Payload } from './types';
const stateNames:Record<string,string>={verified:'实测支持',documented:'资料支持（未实测）',unknown:'未知',incompatible:'明确不兼容'};
const eligibility:Record<string,string>={excluded:'不符合必须条件',needs_verification:'必须条件仍待核实',document_only:'可继续核查，尚未实测'};
const percent=(n:number|null)=>n===null?'N/A':`${Math.round(n)}%`;
function FactSource({fact,body}:{fact:Fact|null;body:Body}){
 const source=fact?.source?body.catalog.sources[fact.source]:null;
 return <>{fact?.note??'尚未关联可核实的Harness能力。'}{source&&<div className="fit-source-link"><span>{source.title} · 查阅于 {source.checkedAt}</span><code>{source.url}</code><button className="button text-button" onClick={()=>void navigator.clipboard.writeText(source.url).then(()=>notify('来源链接已复制')).catch(()=>notify('复制失败，请选择链接文字复制'))}>复制链接</button></div>}</>;
}
export function FitReport({body,criteria,onSource}:{body:Body;criteria:Payload['criteria'];onSource:(r:Requirement)=>void}){
 const provisional=body.requirements.filter(r=>r.status==='candidate').length;
 const allowed=body.report.results.filter(r=>r.eligibility==='document_only');
 const pool=allowed.length?allowed:body.report.results.filter(r=>r.eligibility!=='excluded');
 const lower=Math.max(0,...pool.map(r=>r.lowerPercent??0));
 const suggested=pool.filter(r=>r.lowerPercent===lower&&lower>0).map(r=>body.catalog.entries.find(e=>e.id===r.harnessId)!.name);
 return <div className="fit-report">
  <header className="fit-report-intro"><h2>项目与工具的静态适配</h2><p>依据项目要求与官方资料推断；{provisional?`仍有${provisional}条线索待确认。`:'需求确认状态已记录。'}这里的比例表示资料支持范围，不是编程成功率。</p></header>
  <p>{suggested.length?`可优先核查：${suggested.join('、')}。在未排除的候选中，优先考虑必须条件有资料支持、加权支持下界较高的方案；并列均保留。`:'证据不足，暂不推荐具体工具。请先确认要求与关联条件。'}</p>
  <div className="fit-comparison" role="region" aria-label="Harness静态适配比较" tabIndex={0}><table><caption className="sr-only">资料支持比例及硬性兼容性</caption><thead><tr><th>Harness</th><th>必须条件</th><th>资料支持范围</th><th>证据覆盖</th><th>本机运行</th></tr></thead><tbody>{body.report.results.map(result=>{
   const entry=body.catalog.entries.find(e=>e.id===result.harnessId)!;
   return <tr key={entry.id}><th scope="row">{entry.name}</th><td><FitStatus tone={result.eligibility==='document_only'?'neutral':'attention'}>{eligibility[result.eligibility]}</FitStatus></td><td>{percent(result.lowerPercent)} – {percent(result.upperPercent)}</td><td>{percent(result.evidenceCoverage)}</td><td><FitStatus tone="attention">尚未验证</FitStatus></td></tr>;
  })}</tbody></table></div>
  <details className="fit-method"><summary>比例怎么算，未知项怎么处理</summary><p>下界＝资料支持的权重÷总权重；上界＝（资料支持＋未知）的权重÷总权重。相同条件重复出现只计一次；未关联条件的项目背景、已忽略项不计分。权重为0不计比例，但“必须满足”仍检查。未实测项不会显示成已验证兼容。</p><p>目录 {body.catalog.version} · 引擎 {body.report.engineVersion}</p><code>{body.catalogHash}</code></details>
  {body.report.results.map(result=>{
   const entry=body.catalog.entries.find(e=>e.id===result.harnessId)!;
   return <details className="fit-harness" key={entry.id}><summary><strong>{entry.name}</strong><FitStatus tone={result.eligibility==='document_only'?'neutral':'attention'}>{eligibility[result.eligibility]}</FitStatus><span>查看逐项依据</span></summary>
    <dl className="fit-catalog-details"><dt>版本与渠道</dt><dd><div className="fit-meta-line">{entry.version?<span>{entry.version}</span>:<FitStatus tone="attention">安装版本未知</FitStatus>}<span>{entry.channel}</span></div></dd><dt>系统</dt><dd>{entry.operatingSystems}</dd><dt>模型与Provider</dt><dd>{entry.modelAccess}</dd><dt>认证与费用</dt><dd>{entry.authenticationAndCost}</dd><dt>上下文</dt><dd>{entry.contextMechanism}</dd><dt>工具与终端</dt><dd>{entry.toolMechanism}</dd></dl>
    <h3>已关联条件</h3>
    {result.rows.filter(row=>body.requirements.find(r=>r.id===row.requirementId)?.criterion).map(row=>{const req=body.requirements.find(r=>r.id===row.requirementId)!;return <div className="fit-result-row" key={row.requirementId}><div><FitStatus tone={row.state==='verified'?'success':row.state==='documented'?'neutral':'attention'}>{stateNames[row.state]}</FitStatus><span>权重 {row.weight}{row.weight===0?'（不重复计分或已设为0）':''}</span>{req.hard&&<span>必须满足</span>}{row.provisional&&<FitStatus tone="attention">需求待确认</FitStatus>}</div><p>{req.value}</p><FactSource fact={row.fact} body={body}/>{req.evidence&&<button className="button text-button" onClick={()=>onSource(req)}>查看项目依据 · {req.evidence.path}:{req.evidence.startLine}</button>}</div>;})}
    <h3>完整能力目录</h3><dl className="fit-catalog-details">{Object.entries(entry.facts).map(([key,fact])=><div key={key}><dt>{criteria[key]?.label??key}</dt><dd><FitStatus tone={fact.value===true?'neutral':'attention'}>{fact.value===null?'未知':fact.value?'资料声明支持':'资料明确不支持'}</FitStatus><div className="fit-status-copy"><FactSource fact={fact} body={body}/></div></dd></div>)}</dl>
   </details>;
  })}
  <p>已保存分析可在“组合实验”中查看实际任务表现。</p>
 </div>;
}

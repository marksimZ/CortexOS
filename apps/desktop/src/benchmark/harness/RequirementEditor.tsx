import { SelectMenu } from '../../components/SelectMenu';
import { statuses,type Payload,type Requirement } from './types';
export function RequirementEditor({requirements,data,disabled,onChange,onSource}:{requirements:Requirement[];data:Payload;disabled:boolean;onChange:(r:Requirement)=>void;onSource:(r:Requirement)=>void}){
 return <div className="fit-requirements">{data.categories.map(category=><details className="fit-category" key={category.id} open={category.id==='languages'}>
  <summary><h3>{category.label}</h3><span>{requirements.filter(r=>r.category===category.id).length} 项</span></summary>
  {requirements.filter(r=>r.category===category.id).map(r=><div className={`fit-requirement ${r.status==='rejected'?'fit-ignored':''}`} data-requirement-id={r.id} key={r.id}>
   <label className="profile-field"><span className="sr-only">{category.label}要求</span><textarea rows={2} maxLength={4096} disabled={disabled} value={r.value} placeholder="未找到明确要求，可在这里补充" onChange={e=>onChange({...r,value:e.target.value,status:'edited'})}/></label>
   <div className="fit-requirement-controls">
    <label>状态<SelectMenu aria-label={`${category.label}确认状态`} disabled={disabled} value={r.status} onChange={e=>{const status=e.target.value as Requirement['status'];onChange({...r,status,hard:['confirmed','edited'].includes(status)?r.hard:false});}}>{Object.entries(statuses).map(([id,label])=><option key={id} value={id}>{label}</option>)}</SelectMenu></label>
    <label>关联条件<SelectMenu aria-label={`${category.label}关联条件`} disabled={disabled} value={r.criterion??''} onChange={e=>onChange({...r,criterion:e.target.value||null})}><option value="">仅记录项目背景</option>{Object.entries(data.criteria).map(([id,c])=><option key={id} value={id}>{c.label}</option>)}</SelectMenu></label>
    <label className="fit-hard"><input type="checkbox" checked={r.hard} disabled={disabled||!['confirmed','edited'].includes(r.status)} onChange={e=>onChange({...r,hard:e.target.checked})}/>必须满足</label>
   </div>
   {r.evidence?<details className="fit-evidence"><summary>查看依据 · {r.evidence.path}:{r.evidence.startLine}</summary><blockquote>{r.evidence.excerpt}</blockquote><button className="button secondary" onClick={()=>onSource(r)}>打开原文位置</button>{r.status==='edited'&&<p>当前内容由用户修改；以上引用保留原始线索，不代表文档已写出修改后的要求。</p>}</details>:<p>{r.value?'用户补充，未附文档证据。':'资料缺少这类信息，保持未知。'}</p>}
  </div>)}
 </details>)}</div>;
}

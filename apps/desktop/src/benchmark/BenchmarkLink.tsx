import { useRef, useState } from "react";
import { Link2, X } from "lucide-react";
import { AppDialog, openDialog } from "../components/AppDialog";
import { navigate } from "../navigation";
import { reportLink, reportParams } from "./reportNavigation";

export function BenchmarkLink() {
  const dialog = useRef<HTMLDialogElement>(null);
  const [value,setValue] = useState(""),[error,setError]=useState("");
  return <><button className="button text-button" onClick={()=>openDialog(dialog.current)}><Link2 size={16} aria-hidden="true" />打开定位链接</button>
    <AppDialog ref={dialog} className="form-dialog panel-dialog" aria-labelledby="benchmark-link-title"><form className="panel-form" onSubmit={event=>{
      event.preventDefault();
      try { const hash=reportLink(value.trim()); navigate("benchmark",Object.fromEntries(reportParams(hash))); dialog.current?.close(); setError(""); }
      catch(e){setError(e instanceof Error?e.message:"定位链接无效。");}
    }}><header className="panel-header"><h2 id="benchmark-link-title">打开评测定位链接</h2><button className="icon-button" type="button" aria-label="关闭定位链接" onClick={()=>dialog.current?.close()}><X size={18} aria-hidden="true" /></button></header>
      <div className="panel-body"><label className="field">定位链接<input aria-label="评测定位链接" value={value} onChange={e=>setValue(e.target.value)} placeholder="#/benchmark?..." required maxLength={8192} /></label>{error&&<p className="form-error" role="alert">{error}</p>}</div>
      <footer className="panel-footer dialog-actions"><button className="button secondary" type="button" onClick={()=>dialog.current?.close()}>取消</button><button className="button primary">打开</button></footer>
    </form></AppDialog></>;
}

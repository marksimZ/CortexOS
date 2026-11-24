import type { RefObject } from "react";
import { X } from "lucide-react";
import { navigationHash, type PageName } from "../navigation";
import { AppDialog } from "./AppDialog";

const model = navigationHash("settings", { section: "model" });
const cloud = navigationHash("settings", { section: "cloud" });
const data = navigationHash("settings", { section: "data" });
const help = [
  { title: "首次使用", body: "在聊天输入框的「回答模型」中选择本机已安装的模型，或当前工作区已验证生成能力的云端模型。选择「自动」会按工作区运行方案路由。", page: ["chat"], link: [cloud, "配置云端模型"] },
  { title: "模型连不上或列表为空", body: "先确认 Ollama 正在运行，再到模型连接里检测。服务已连接但无模型时，请先在 Ollama 安装一个模型，随后重新检测并保存选择。", page: ["chat", "settings"], link: [model, "检查本机连接"] },
  { title: "知识查找与索引", body: "在知识页用「资料文件」添加文本，再打开对话的知识查找。向量索引未建好时可使用词语查找。索引状态和重建入口在设置的模型连接中。", page: ["knowledge", "graph"], link: [model, "查看索引状态"] },
  { title: "回答依据和历史版本", body: "在回答旁打开回答依据，可定位本次实际用到的文件、知识版本或处理记录。知识页的版本历史可比较和恢复版本。", page: ["chat", "knowledge", "graph"] },
  { title: "云端发送与费用", body: "云端服务可能按输入、输出或请求收费。连接验证和运行前请核对模型与工作区隐私策略。", page: ["settings"], link: [cloud, "打开云端设置"] },
  { title: "备份与恢复", body: "在本地数据中创建工作区或完整备份。恢复时先查看来源和冲突，确认后按提示重启。API 密钥由系统凭据存储保管，跨电脑后需重新设置。", page: ["settings"], link: [data, "查看备份与恢复"] },
  { title: "任务失败、取消或中断", body: "先打开任务中心或原记录看原因。云端请求中断时，先核对用量和历史，再决定是否重试。", page: ["chat", "settings"] },
] as const;

export function WorkspaceHelp({ dialog, page }: { dialog: RefObject<HTMLDialogElement | null>; page: PageName }) {
  const current = help.filter(topic => (topic.page as readonly PageName[]).includes(page));
  const other = help.filter(topic => !current.includes(topic));
  return <AppDialog ref={dialog} className="side-drawer panel-dialog workspace-help" aria-labelledby="workspace-help-title">
    <header className="panel-header">
      <h2 id="workspace-help-title">使用帮助</h2>
      <button className="icon-button" type="button" aria-label="关闭使用帮助" onClick={() => dialog.current?.close()}><X size={18} aria-hidden="true" /></button>
    </header>
    <div className="panel-body">
      {[...current, ...other].map(topic => <details className="field-help" key={topic.title}>
        <summary>{topic.title}</summary>
        <p>{topic.body}</p>
        {"link" in topic && <a className="button text-button" href={topic.link[0]} onClick={() => dialog.current?.close()}>{topic.link[1]}</a>}
      </details>)}
    </div>
  </AppDialog>;
}

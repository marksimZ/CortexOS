import { useId } from "react";
import type { ReminderInput } from "../api/reminders";
import { reminderOccurrences } from "../reminder-draft";
import { SelectMenu } from "./SelectMenu";

const weekdays = ["一", "二", "三", "四", "五", "六", "日"];
const dateTime = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric", month: "2-digit", day: "2-digit", weekday: "short",
  hour: "2-digit", minute: "2-digit", hourCycle: "h23",
});

export function ReminderFields({ value, onChange }: {
  value: ReminderInput;
  onChange: (value: ReminderInput) => void;
}) {
  const id = useId();
  const occurrences = reminderOccurrences(value);
  const isBriefing = value.newsQuery != null;
  return <>
    <div className="field">
      <label htmlFor={id + "-title"}>{isBriefing ? "简报名称" : "提醒内容"}</label>
      <input id={id + "-title"} value={value.title} required
        onChange={event => onChange({ ...value, title: event.target.value })} />
    </div>
    {isBriefing && <div className="field">
      <label htmlFor={id + "-query"}>新闻查询词</label>
      <input id={id + "-query"} value={value.newsQuery ?? ""} required maxLength={300}
        aria-describedby={id + "-query-disclosure"}
        onChange={event => onChange({ ...value, newsQuery: event.target.value })} />
      <p className="inline-help" id={id + "-query-disclosure"}>
        到期时，应用会将此查询词发送给 Bing News，保存最多 5 条新闻的标题、链接和摘要。应用运行时即可查询，无需连接模型。
      </p>
    </div>}
    <div className="field">
      <label htmlFor={id + "-kind"}>重复规则</label>
      <SelectMenu id={id + "-kind"} value={value.kind}
        onChange={event => onChange({ ...value, kind: event.target.value as ReminderInput["kind"] })}>
        <option value="once">仅一次</option>
        <option value="daily">每天</option>
        <option value="weekdays">每周一至周五</option>
        <option value="weekly">每周</option>
      </SelectMenu>
    </div>
    {value.kind === "weekly" && <div className="field">
      <label htmlFor={id + "-weekday"}>星期</label>
      <SelectMenu id={id + "-weekday"} value={value.weekday}
        onChange={event => onChange({ ...value, weekday: Number(event.target.value) })}>
        {weekdays.map((day, index) => <option key={day} value={index + 1}>星期{day}</option>)}
      </SelectMenu>
    </div>}
    <div className="field">
      <label htmlFor={id + "-date"}>{value.kind === "once" ? "日期" : "开始日期"}</label>
      <input id={id + "-date"} type="date" value={value.date} required
        onChange={event => onChange({ ...value, date: event.target.value })} />
    </div>
    <div className="field">
      <label htmlFor={id + "-time"}>本地时间</label>
      <input id={id + "-time"} type="time" value={value.time} required
        onChange={event => onChange({ ...value, time: event.target.value })} />
    </div>
    <div className="field" role="status">
      <strong>{value.kind === "once" ? "触发时间" : isBriefing ? "接下来三次查询" : "接下来三次提醒"}</strong>
      {occurrences.length ? <ul>{occurrences.map(date =>
        <li key={date.getTime()}><time dateTime={date.toISOString()}>{dateTime.format(date)}</time></li>)}</ul>
        : <p>{value.date && value.time ? "请选择可触发的未来时间。" : "请填写日期和时间。"}</p>}
    </div>
  </>;
}

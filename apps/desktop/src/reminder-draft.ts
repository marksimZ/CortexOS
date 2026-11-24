import type { ReminderInput } from "./api/reminders";

const pad = (value: number) => String(value).padStart(2, "0");
const dateText = (date: Date) => `${String(date.getFullYear()).padStart(4, "0")}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
const dayNumber = (date: Date) => date.getDay() || 7;

export function newReminder(now = new Date()): ReminderInput {
  return { title: "", kind: "once", date: dateText(now), time: "", weekday: dayNumber(now) };
}

export function reminderOccurrences(input: ReminderInput, now = new Date()): Date[] {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(input.date) || !/^\d{2}:\d{2}$/.test(input.time)) return [];
  const start = new Date(`${input.date}T00:00`);
  if (Number.isNaN(start.getTime()) || start.getFullYear() < 1 || dateText(start) !== input.date) return [];
  const [hour, minute] = input.time.split(":").map(Number);
  if (hour > 23 || minute > 59) return [];
  const day = start > now || input.kind === "once" ? start : new Date(`${dateText(now)}T00:00`);
  const matches: Date[] = [];
  const days = input.kind === "once" ? 1 : 29;
  for (let offset = 0; offset < days && matches.length < 3; offset++) {
    const weekday = dayNumber(day);
    const occurrence = new Date(day);
    occurrence.setHours(hour, minute, 0, 0);
    if (occurrence.getFullYear() <= 9999 && occurrence > now && occurrence.getHours() === hour && occurrence.getMinutes() === minute &&
        (input.kind !== "weekdays" || weekday <= 5) &&
        (input.kind !== "weekly" || weekday === input.weekday)) {
      matches.push(occurrence);
    }
    day.setDate(day.getDate() + 1);
  }
  return matches;
}

const digits = "零一二三四五六七八九";
const countText = "[0-9零一二三四五六七八九十两百]{1,8}";
function number(text: string): number {
  if (/^\d+$/.test(text)) return Number(text);
  if (/百.*百|十.*十/.test(text)) return NaN;
  if (text === "两") return 2;
  if (text.length === 1) return digits.indexOf(text);
  for (const [unit, size] of [["百", 100], ["十", 10]] as const) {
    const index = text.indexOf(unit);
    if (index < 0) continue;
    const rest = text.slice(index + 1).replace(/^零/, "");
    return (index === 0 ? 1 : number(text.slice(0, index))) * size + (rest ? number(rest) : 0);
  }
  return NaN;
}

// ponytail: recognize common one-line Chinese requests and leave uncertain dates and times editable.
export function parseReminder(text: string, now = new Date()): ReminderInput | null {
  const sentence = text.trim().replace(/[。！!]+$/, "");
  if (/[\n\r？?]/.test(sentence)) return null;
  const request = sentence.match(/^(.*?)(?:请)?提醒我[：:，,\s]*(.+)$/);
  if (request) {
    const title = request[2].trim();
    return title ? parseSchedule(request[1], { ...newReminder(now), title }, now) : null;
  }
  const briefing = sentence.match(/^(请?(?:每个工作日|工作日|每天|每日|每(?:个)?(?:周|星期)[一二三四五六日天])[^。；;]*?)(?:帮我)?(?:整理|汇总)\s*(.{0,300}?(?:新闻|资讯|简报|动态))$/);
  if (!briefing) return null;
  const newsQuery = briefing[2].trim();
  const title = Array.from(newsQuery).slice(0, 120).join("");
  return parseSchedule(briefing[1], { ...newReminder(now), title, newsQuery }, now);
}

function parseSchedule(text: string, input: ReminderInput, now: Date): ReminderInput | null {
  const schedule = text.replace(/^请/, "").replace(/[，,\s]+/g, "").replace(/：/g, ":");
  if (/[。！!；;：:]/.test(schedule.replace(/\d:\d/g, ""))) return null;
  const relative = schedule.match(new RegExp(`^(${countText})(分钟|小时|天)后$`))
    ?? schedule.match(new RegExp(`^过(${countText})(分钟|小时|天)$`));
  if (relative) {
    const minutes = number(relative[1]) * ({ 分钟: 1, 小时: 60, 天: 1440 }[relative[2]] ?? 0);
    const due = new Date(Math.ceil(now.getTime() / 60_000) * 60_000 + minutes * 60_000);
    if (minutes > 0 && !Number.isNaN(due.getTime()) && due.getFullYear() <= 9999) {
      input.date = dateText(due);
      input.time = `${pad(due.getHours())}:${pad(due.getMinutes())}`;
    }
    return input;
  }

  const prefix = schedule.match(/^(每个工作日|工作日|每天|每日|(?:每(?:个)?|下(?:个)?)?(?:周|星期)[一二三四五六日天]|今天|今晚|明天|明早|后天|\d{4}-\d{2}-\d{2})/)?.[0] ?? "";
  const namedDay = prefix.match(/(?:周|星期)([一二三四五六日天])$/)?.[1];
  const weekday = namedDay === "日" || namedDay === "天" ? 7 : digits.indexOf(namedDay ?? "");
  if (prefix.endsWith("工作日")) input.kind = "weekdays";
  else if (prefix === "每天" || prefix === "每日") input.kind = "daily";
  else if (namedDay && prefix.startsWith("每")) {
    input.kind = "weekly";
    input.weekday = weekday;
  } else if (namedDay) {
    const due = new Date(now);
    const difference = weekday - dayNumber(now);
    due.setDate(due.getDate() + (prefix.startsWith("下") || difference < 0 ? difference + 7 : difference));
    input.date = dateText(due);
  } else if (/^\d/.test(prefix)) input.date = prefix;
  else if (/^(明天|明早|后天)$/.test(prefix)) {
    const due = new Date(now);
    due.setDate(due.getDate() + (prefix === "后天" ? 2 : 1));
    input.date = dateText(due);
  }

  const time = schedule.slice(prefix.length);
  const clock = time.match(new RegExp(
    `^(凌晨|早上|上午|中午|下午|晚上)?(${countText})(?:[点时](半|(?:${countText})分?)?|[:：]([0-9]{2}))$`,
  ));
  if (!clock) {
    if (/^(?:凌晨|早上|上午|下午|中午|晚上|晚些时候)?$/.test(time)) return input;
    if (!prefix && /^(?:下次|稍后|待会儿?|一会儿?|过会儿?|有空时|下(?:个)?(?:周|星期))(?:凌晨|早上|上午|下午|中午|晚上)?$/.test(time)) {
      return { ...input, date: "" };
    }
    return null;
  }
  const period = clock[1] ?? (prefix === "今晚" ? "晚上" : "");
  let hour = number(clock[2]);
  const minute = clock[3] === "半" ? 30 : number((clock[3] ?? clock[4] ?? "0").replace(/分$/, ""));
  if ((!period && clock[4] === undefined && hour >= 1 && hour <= 12) ||
      (period === "晚上" && hour === 12) ||
      (period === "中午" && (hour < 11 || hour > 13))) return input;
  if (period === "凌晨" && hour === 12) hour = 0;
  if (/^(下午|晚上)$/.test(period) && hour >= 1 && hour < 12) hour += 12;
  if (hour >= 0 && hour < 24 && minute >= 0 && minute < 60) input.time = `${pad(hour)}:${pad(minute)}`;
  return input;
}

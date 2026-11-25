import { SelectMenu } from "./SelectMenu";
import { useEffect, useId, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";
import { preference, remember } from "../studio";

type Appearance = "system" | "light" | "dark";
function savedAppearance(): Appearance {
  const stored = preference("appearance");
  return stored === "dark" || stored === "light" ? stored : "system";
}
const media = window.matchMedia("(prefers-color-scheme: dark)");
function apply(value: Appearance) {
  const scheme = value === "system" ? (media.matches ? "dark" : "light") : value;
  document.documentElement.dataset.theme = value;
  document.documentElement.dataset.colorScheme = scheme;
}
apply(savedAppearance());

export function AppearanceControl() {
  const id = useId();
  const [appearance, setAppearance] = useState<Appearance>(savedAppearance);
  useEffect(() => {
    apply(appearance);
    const update = () => apply(appearance);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [appearance]);
  const Icon = appearance === "system" ? Monitor : appearance === "dark" ? Moon : Sun;
  return <label className="appearance-control" title="切换外观主题（跟随系统 / 浅色 / 深色）" htmlFor={id}>
    <Icon size={16} aria-hidden="true" />
    <SelectMenu id={id} aria-label="外观主题" value={appearance} onChange={event => {
      const value = event.target.value as Appearance;
      setAppearance(value);
      remember("appearance", value);
    }}>
      <option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option>
    </SelectMenu>
  </label>;
}

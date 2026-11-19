import { useMemo, useSyncExternalStore } from "react";
export type PageName = "chat" | "knowledge" | "graph" | "settings" | "benchmark";
const subscribe = (change: () => void) => {
  window.addEventListener("hashchange", change);
  window.addEventListener("popstate", change);
  return () => { window.removeEventListener("hashchange", change); window.removeEventListener("popstate", change); };
};
export function useNavigation() {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  return useMemo(() => {
    const [path, query = ""] = hash.replace(/^#\//, "").split("?");
    const params = new URLSearchParams(query);
    const page: PageName = ["chat", "knowledge", "graph", "settings", "benchmark"].includes(
      path,
    )
      ? (path as PageName)
      : "chat";
    const number = Number(params.get("version"));
    const citation = params.has("citation") ? Number(params.get("citation")) : -1;
    return {
      page,
      personalized: params.get("view") === "personalized",
      hash,
      workspace: params.get("workspace"),
      conversation: params.get("conversation"),
      run: params.get("run"),
      node: params.get("node"),
      event: params.get("event"),
      message: params.get("message"),
      citation: Number.isSafeInteger(citation) && citation >= 0 ? citation : undefined,
      version: Number.isSafeInteger(number) && number > 0 ? number : undefined,
      mode: params.get("mode") === "execution" ? "execution" : "knowledge",
      back: /^(#\/graph\?|#\/chat\?|#\/benchmark\?)/.test(params.get("back") ?? "")
        ? params.get("back")
        : null,
    };
  }, [hash]);
}
export function navigationHash(
  page: PageName,
  values: Record<string, string | number | null | undefined> = {},
) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(values))
    if (value !== null && value !== undefined) params.set(key, String(value));
  return `#/${page}${params.size ? `?${params}` : ""}`;
}
export function navigate(page: PageName, values: Record<string, string | number | null | undefined> = {}, options: { replace?: boolean; state?: unknown } = {}) {
  const hash = navigationHash(page, values);
  if (hash === window.location.hash && !options.replace) return;
  const oldURL = window.location.href;
  window.history[options.replace ? "replaceState" : "pushState"](options.state ?? null, "", hash);
  window.dispatchEvent(new HashChangeEvent("hashchange", { oldURL, newURL: window.location.href }));
}

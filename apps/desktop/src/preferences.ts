export function preference(key: string): string | null {
  try {
    return localStorage.getItem(`cortexos.${key}`);
  } catch {
    return null;
  }
}
export function remember(key: string, value: string) {
  try {
    localStorage.setItem(`cortexos.${key}`, value);
  } catch {
    /* Optional view preference; domain data stays in SQLite. */
  }
}

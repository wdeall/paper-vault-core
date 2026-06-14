// Tauri / 浏览器工具
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function basename(p: string): string {
  if (!p) return "";
  return p.split(/[\\/]/).pop() ?? p;
}

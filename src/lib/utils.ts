// 通用工具函数

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/** 时间戳 → 友好日期 */
export function formatDate(ts: number | null | undefined): string {
  if (!ts) return "—";
  const d = new Date(ts);
  return d.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

/** 作者截断：3 人以内全显示，更多则 et al. */
export function formatAuthors(authors: string[]): string {
  if (authors.length === 0) return "（未填）";
  if (authors.length <= 3) return authors.join(", ");
  return authors.slice(0, 3).join(", ") + " et al.";
}

/** 防抖 */
export function debounce<T extends (...args: unknown[]) => void>(
  fn: T,
  ms: number,
): T {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return ((...args: unknown[]) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  }) as T;
}

/** 复制到剪贴板 */
export async function copyToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/** 文件名（去掉路径） */
export function basename(p: string): string {
  return p.split(/[\\/]/).pop() || p;
}

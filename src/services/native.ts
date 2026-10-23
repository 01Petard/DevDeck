import { invoke } from "@tauri-apps/api/core";
import type { CommandError, PortListener, HealthCheckResult } from "../types/devdock";

export function listListeners(): Promise<PortListener[]> {
  return invoke<PortListener[]>("list_listeners");
}

export function checkHttpHealth(url: string): Promise<HealthCheckResult> {
  return invoke<HealthCheckResult>("check_http_health", { url });
}

export function normalizeScanError(error: unknown): CommandError {
  const code = error && typeof error === "object" && "code" in error ? error.code : null;
  if (code === "SCAN_TIMEOUT") {
    return { code, message: "扫描超时，结果未更新。请稍后刷新。" };
  }
  if (code === "SCAN_FAILED") {
    return { code, message: "扫描失败，结果未更新。请重试；部分进程可能受权限限制。" };
  }
  return { code: "INTERNAL_ERROR", message: "暂时无法扫描端口，结果未更新。请稍后刷新。" };
}

import { invoke } from "@tauri-apps/api/core";
import type { CommandError, PortListener, HealthCheckResult, TerminateRequest, TerminateResult } from "../types/devdock";

export function listListeners(): Promise<PortListener[]> {
  return invoke<PortListener[]>("list_listeners");
}

export function checkHttpHealth(url: string): Promise<HealthCheckResult> {
  return invoke<HealthCheckResult>("check_http_health", { url });
}

export function terminateListener(request: TerminateRequest): Promise<TerminateResult> {
  return invoke<TerminateResult>("terminate_listener", { request });
}

export function normalizeTerminateError(error: unknown): CommandError {
  const code = error && typeof error === "object" && "code" in error ? error.code : null;
  switch (code) {
    case "INVALID_ARGUMENT": return { code, message: "操作参数无效或已有操作进行中。请刷新并重新选择。" };
    case "PROCESS_NOT_FOUND": return { code, message: "进程已不存在，未发送信号。" };
    case "PROCESS_IDENTITY_CHANGED": return { code, message: "进程身份或监听端口已变化，未发送信号。请重新选择。" };
    case "PROCESS_UNVERIFIABLE": return { code, message: "无法验证进程身份与监听端口，未发送信号。" };
    case "PROCESS_PROTECTED": return { code, message: "不能结束 DevDock 自身或系统特殊进程，未发送信号。" };
    case "PERMISSION_DENIED": return { code, message: "没有结束该进程的权限，未发送信号。" };
    case "SIGNAL_FAILED": return { code, message: "未能发送 SIGTERM，请刷新检查当前状态。" };
    default: return { code: "INTERNAL_ERROR", message: "无法确认操作结果，请刷新检查进程状态。" };
  }
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

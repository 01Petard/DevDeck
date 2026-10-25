import { computed, ref, shallowRef } from "vue";
import { listenerKey } from "./useListeners.ts";
import { normalizeTerminateError, terminateListener } from "../services/native.ts";
import type { PortListener } from "../types/devdock.ts";

export interface TerminationTarget { listener: PortListener; ports: number[] }
export interface TerminationNotice { kind: "exited" | "signal_sent" | "failed"; message: string }

export function useTermination(selected: () => PortListener | null, listeners: () => PortListener[], stale: () => boolean,
  afterAttempt: () => Promise<void>, terminate = terminateListener) {
  const target = shallowRef<TerminationTarget | null>(null);
  const sending = ref(false);
  const notice = shallowRef<TerminationNotice | null>(null);
  const disabledReason = computed(() => {
    const listener = selected();
    if (sending.value) return "正在等待终止操作结果。";
    if (stale()) return "监听快照已过期，请刷新后重新选择。";
    if (!listener) return "请先选择一条真实监听记录。";
    if (!Number.isInteger(listener.pid) || listener.pid <= 1 || listener.pid > 2147483647) return "此进程受保护，不能安全终止。";
    if (!Number.isSafeInteger(listener.processStartTime) || (listener.processStartTime ?? 0) <= 0) return "无法验证进程身份，不能安全终止。";
    return null;
  });
  function open() {
    const listener = selected();
    if (disabledReason.value || !listener) return;
    notice.value = null;
    target.value = { listener: { ...listener }, ports: [...new Set(listeners().filter(item => item.pid === listener.pid).map(item => item.port))].sort((a, b) => a - b) };
  }
  function cancel() { if (!sending.value) target.value = null; }
  async function confirm() {
    const confirmed = target.value;
    if (!confirmed || sending.value) return;
    const listener = selected();
    if (disabledReason.value || !listener || listenerKey(listener) !== listenerKey(confirmed.listener)) {
      notice.value = { kind: "failed", message: "监听记录已过期或选择已变化，未发送信号。请刷新并重新选择。" };
      target.value = null;
      return;
    }
    sending.value = true;
    const identity = `${confirmed.listener.processName || "进程"}（PID ${confirmed.listener.pid}，端口 ${confirmed.listener.port}）`;
    try {
      const result = await terminate({ pid: confirmed.listener.pid, processStartTime: confirmed.listener.processStartTime!, port: confirmed.listener.port });
      notice.value = result.signalSent ? {
        kind: result.processExited ? "exited" : "signal_sent",
        message: `${identity}：${result.processExited ? "已发送 SIGTERM，并确认原进程已退出。" : "已发送 SIGTERM，2 秒内尚未确认原进程退出。"}`,
      } : { kind: "failed", message: `${identity}：未能确认信号是否发送，请刷新检查状态。` };
    } catch (cause) {
      notice.value = { kind: "failed", message: `${identity}：${normalizeTerminateError(cause).message}` };
    } finally {
      target.value = null;
      try { await afterAttempt(); }
      catch { if (notice.value) notice.value = { ...notice.value, message: `${notice.value.message} 请手动刷新监听。` }; }
      sending.value = false;
    }
  }
  return { target, sending, notice, disabledReason, open, cancel, confirm };
}

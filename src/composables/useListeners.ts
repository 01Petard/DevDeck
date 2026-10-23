import { computed, ref, shallowRef, watch } from "vue";
import { listListeners, normalizeScanError } from "../services/native.ts";
import type { CommandError, PortListener } from "../types/devdock.ts";

export function listenerKey(listener: PortListener): string {
  return JSON.stringify([listener.pid, listener.processStartTime, listener.address, listener.port, listener.ipVersion, listener.protocol]);
}

export function useListeners(load = listListeners, now = Date.now) {
  const listeners = shallowRef<PortListener[]>([]);
  const query = ref("");
  const selectedKey = ref<string | null>(null);
  const isRefreshing = ref(false);
  const lastScanAt = ref<number | null>(null);
  const error = shallowRef<CommandError | null>(null);
  let lastAttemptAt = -Infinity;
  const isStale = computed(() => isRefreshing.value || error.value !== null || lastScanAt.value === null);
  const filteredListeners = computed(() => {
    const search = query.value.trim().toLowerCase();
    return listeners.value.filter(listener => !search || String(listener.port).includes(search)
      || String(listener.pid).includes(search) || listener.processName.toLowerCase().includes(search));
  });
  const selectedListener = computed<PortListener | null>(() => isStale.value ? null
    : filteredListeners.value.find(listener => listenerKey(listener) === selectedKey.value) ?? null);
  watch(query, () => { selectedKey.value = null; }, { flush: "sync" });

  async function refresh(reason: "manual" | "focus" = "manual") {
    const attemptedAt = now();
    const elapsed = attemptedAt - lastAttemptAt;
    if (isRefreshing.value || (elapsed >= 0 && elapsed < (reason === "focus" ? 2000 : 300))) return;
    lastAttemptAt = attemptedAt;
    isRefreshing.value = true;
    error.value = null;
    try {
      const snapshot = [...await load()].sort((a, b) => a.port - b.port || a.pid - b.pid
        || a.address.localeCompare(b.address) || (a.ipVersion ?? 0) - (b.ipVersion ?? 0));
      listeners.value = snapshot;
      lastScanAt.value = now();
      if (!snapshot.some(listener => listenerKey(listener) === selectedKey.value)) selectedKey.value = null;
    } catch (cause) {
      error.value = normalizeScanError(cause);
      selectedKey.value = null;
    } finally {
      isRefreshing.value = false;
    }
  }
  function selectListener(key: string) {
    if (!isStale.value && filteredListeners.value.some(listener => listenerKey(listener) === key)) {
      selectedKey.value = key;
    }
  }
  return { listeners, query, selectedKey, isRefreshing, lastScanAt, error, isStale, filteredListeners, selectedListener, refresh, selectListener };
}

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import ListenerList from "./components/ListenerList.vue";
import ListenerDetail from "./components/ListenerDetail.vue";
import { useListeners } from "./composables/useListeners";

const {
  listeners, query, selectedKey, isRefreshing, lastScanAt, error,
  isStale, filteredListeners, selectedListener, refresh, selectListener,
} = useListeners();
const lastScanLabel = computed(() => lastScanAt.value === null ? "尚未完成扫描"
  : `上次成功扫描 ${new Date(lastScanAt.value).toLocaleTimeString("zh-CN", { hour12: false })}`);
let unlisten: UnlistenFn | undefined;
let disposed = false;
function onFocus() { void refresh("focus"); }

onMounted(async () => {
  void refresh();
  if (!isTauri()) {
    window.addEventListener("focus", onFocus);
    return;
  }
  try {
    const stop = await getCurrentWindow().onFocusChanged(({ payload }) => { if (payload) onFocus(); });
    if (disposed) stop();
    else unlisten = stop;
  } catch {
    if (!disposed) window.addEventListener("focus", onFocus);
  }
});
onBeforeUnmount(() => {
  disposed = true;
  unlisten?.();
  window.removeEventListener("focus", onFocus);
});
</script>

<template>
  <main class="app-shell" aria-labelledby="app-title">
    <header class="toolbar">
      <h1 id="app-title">DevDock</h1>
      <label class="search-field">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5" /><path d="m16 16 4.5 4.5" /></svg>
        <input v-model="query" type="search" aria-label="搜索端口、PID或进程名" placeholder="搜索端口、PID或进程名" autocomplete="off" :spellcheck="false" />
      </label>
      <button type="button" class="refresh-button" :disabled="isRefreshing" :aria-busy="isRefreshing" @click="refresh()">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5M19 11a7 7 0 0 0-12-4L4 10m16 4-3 3a7 7 0 0 1-12-4" /></svg>
        {{ isRefreshing ? '扫描中…' : '刷新' }}
      </button>
    </header>
    <div class="scan-status-bar" role="status" aria-live="polite">
      <span :class="{ 'outdated-text': isStale }">
        <template v-if="isRefreshing">{{ lastScanAt === null ? '正在读取本机 TCP 监听…' : '正在刷新 · 上次快照已过期' }}</template>
        <template v-else-if="error">{{ lastScanAt === null ? '监听状态未知' : '扫描失败 · 数据已过期' }}</template>
        <template v-else-if="lastScanAt !== null">{{ listeners.length }} 条 TCP 监听记录</template>
        <template v-else>准备扫描</template>
      </span>
      <span>{{ lastScanLabel }}</span>
    </div>
    <p v-if="error" class="error-banner" role="alert">{{ error.message }}<span v-if="lastScanAt !== null">当前保留上次快照，数据已过期。</span></p>
    <div class="workspace">
      <ListenerList :listeners="filteredListeners" :selected-key="selectedKey" :stale="isStale" :loading="isRefreshing" :has-snapshot="lastScanAt !== null" :failed="error !== null" :searching="query.trim().length > 0" @select="selectListener" />
      <ListenerDetail :listener="selectedListener" :stale="isStale" :loading="isRefreshing" :has-listeners="listeners.length > 0" />
    </div>
    <footer class="app-footer">仅显示当前权限可见的 TCP LISTEN 快照。</footer>
  </main>
</template>

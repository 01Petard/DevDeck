<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import ListenerList from "./components/ListenerList.vue";
import ListenerDetail from "./components/ListenerDetail.vue";
import { useListeners } from "./composables/useListeners";
import { listenerKey, otherListeners } from "./composables/useListeners";
import { useFavorites } from "./composables/useFavorites";
import { useHealth } from "./composables/useHealth";
import FavoriteEditor from "./components/FavoriteEditor.vue";
import FavoriteSummary from "./components/FavoriteSummary.vue";
import type { FavoritePort } from "./types/devdock";
import { useTermination } from "./composables/useTermination";
import TerminateDialog from "./components/TerminateDialog.vue";

const {
  listeners, query, selectedKey, isRefreshing, lastScanAt, error,
  isStale, filteredListeners, selectedListener, refresh, selectListener,
} = useListeners();
const { target: terminationTarget, sending, notice: terminationNotice, disabledReason: terminationDisabledReason,
  open: openTermination, cancel: cancelTermination, confirm: confirmTermination } = useTermination(
  () => selectedListener.value, () => listeners.value, () => isStale.value,
  async () => { selectedKey.value = null; await refresh("action"); },
);
const { favorites, ready, loading: loadingFavorites, saving, error: settingsError, read, save, remove } = useFavorites();
const visibleOtherListeners = computed(() => otherListeners(filteredListeners.value, favorites.value));
const { states: healthStates, busyPorts, run: runHealth, invalidate } = useHealth(() => ready.value ? favorites.value : []);
const favoritePort = ref<number | null>(null);
const activeFavorite = computed(() => favorites.value.find(item => item.port === (selectedListener.value?.port ?? favoritePort.value)) ?? null);
const favoriteCandidates = computed(() => listeners.value.filter(item => item.port === activeFavorite.value?.port));
const visibleFavorites = computed(() => {
  const search = query.value.trim().toLowerCase();
  return favorites.value.filter(item => !search || String(item.port).includes(search) || item.label.toLowerCase().includes(search)
    || filteredListeners.value.some(listener => listener.port === item.port));
});
const editor = ref<{ initial: FavoritePort; editing: boolean } | null>(null);
const editorError = ref<string | null>(null);
const favoriteActionError = ref<string | null>(null);
function openEditor(favorite?: FavoritePort) {
  editorError.value = null;
  editor.value = { initial: favorite ? { ...favorite } : { port: 0, label: "", healthUrl: null }, editing: !!favorite };
}
async function favoriteCurrentPort() {
  const listener = selectedListener.value;
  if (!listener || !ready.value || saving.value) return;
  favoriteActionError.value = null;
  try { await save({ port: listener.port, label: "", healthUrl: null }, null); }
  catch (cause) { favoriteActionError.value = cause instanceof Error ? cause.message : "收藏保存失败。"; }
}
async function removeCurrentFavorite() {
  const favorite = activeFavorite.value;
  if (!favorite || !ready.value || saving.value) return;
  favoriteActionError.value = null;
  try { await remove(favorite.port); invalidate(favorite.port); favoritePort.value = null; }
  catch (cause) { favoriteActionError.value = cause instanceof Error ? cause.message : "取消收藏失败。"; }
}
async function saveEditor(favorite: FavoritePort) {
  try {
    const previous = favorites.value.find(item => item.port === favorite.port);
    await save(favorite, editor.value?.editing ? editor.value.initial.port : null);
    if (previous?.healthUrl !== favorites.value.find(item => item.port === favorite.port)?.healthUrl) invalidate(favorite.port);
    favoritePort.value = favorite.port;
    selectedKey.value = null;
    editor.value = null;
  } catch (cause) { editorError.value = cause instanceof Error ? cause.message : "收藏保存失败。"; }
}
async function removeEditor() {
  if (!editor.value?.editing) return;
  try { const port = editor.value.initial.port; await remove(port); invalidate(port); favoritePort.value = null; editor.value = null; }
  catch (cause) { editorError.value = cause instanceof Error ? cause.message : "取消收藏失败。"; }
}
function chooseFavorite(port: number) { favoritePort.value = port; selectedKey.value = null; }
function chooseListener(key: string) { favoritePort.value = null; selectListener(key); }
function chooseCandidate(key: string) { query.value = ""; selectListener(key); }
watch(query, () => { favoritePort.value = null; });
const lastScanLabel = computed(() => lastScanAt.value === null ? "尚未完成扫描"
  : `上次成功扫描 ${new Date(lastScanAt.value).toLocaleTimeString("zh-CN", { hour12: false })}`);
let unlisten: UnlistenFn | undefined;
let disposed = false;
function onFocus() { if (!terminationTarget.value && !sending.value) void refresh("focus"); }

onMounted(async () => {
  void refresh();
  void read();
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
      <button type="button" class="refresh-button" :disabled="!ready || saving" @click="openEditor()">添加收藏</button>
      <button type="button" class="refresh-button" :disabled="isRefreshing || sending" :aria-busy="isRefreshing" @click="refresh()">
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
    <div v-if="settingsError" class="error-banner" role="alert">{{ settingsError }} <button type="button" class="text-button" :disabled="loadingFavorites" @click="read(true)">{{ loadingFavorites ? '读取中…' : '重新读取' }}</button></div>
    <p v-if="favoriteActionError" class="error-banner" role="alert">{{ favoriteActionError }}</p>
    <p v-if="terminationNotice" class="operation-notice" :class="terminationNotice.kind" role="status" aria-live="polite">{{ terminationNotice.message }}</p>
    <div class="workspace">
      <aside class="sidebar">
        <section class="favorites-panel" aria-label="收藏端口">
          <div class="panel-heading"><h2>收藏</h2><span class="count">{{ visibleFavorites.length }}</span></div>
          <ul v-if="visibleFavorites.length" class="favorite-list">
            <li v-for="favorite in visibleFavorites" :key="favorite.port"><button type="button" class="listener-row" :class="{ selected: activeFavorite?.port === favorite.port }" :aria-pressed="activeFavorite?.port === favorite.port" @click="chooseFavorite(favorite.port)">
              <span class="row-top"><span class="port-number">{{ favorite.port }}</span><span class="process-name">{{ favorite.label || '已收藏' }}</span></span>
              <span class="row-bottom">{{ isStale ? 'TCP 状态未知' : listeners.some(item => item.port === favorite.port) ? 'TCP 监听中' : 'TCP 未监听' }}<span>{{ busyPorts.has(favorite.port) ? 'HTTP 检测中' : '' }}</span></span>
            </button></li>
          </ul>
          <p v-else class="favorites-empty">{{ loadingFavorites ? '正在读取收藏…' : settingsError ? '收藏暂不可用' : query.trim() ? '没有匹配的收藏' : '收藏常用端口，未启动时也会保留。' }}</p>
        </section>
        <ListenerList :listeners="visibleOtherListeners" :selected-key="selectedKey" :stale="isStale" :loading="isRefreshing" :has-snapshot="lastScanAt !== null" :failed="error !== null" :searching="query.trim().length > 0" :has-favorites="visibleFavorites.length > 0" @select="chooseListener" />
      </aside>
      <ListenerDetail :listener="selectedListener" :stale="isStale" :loading="isRefreshing" :has-listeners="listeners.length > 0">
        <template #actions>
          <div class="terminate-action">
            <button type="button" class="refresh-button danger-outline" :disabled="terminationDisabledReason !== null" aria-describedby="terminate-disabled-reason" @click="openTermination">结束进程…</button>
            <p id="terminate-disabled-reason" class="detail-note">{{ terminationDisabledReason || '仅发送 SIGTERM，需要再次确认。' }}</p>
          </div>
        </template>
        <template #before>
          <FavoriteSummary v-if="activeFavorite" :favorite="activeFavorite" :state="healthStates[activeFavorite.port]" :checking="busyPorts.has(activeFavorite.port)" :stale="isStale" :count="favoriteCandidates.length" :ready="ready && !saving" @edit="openEditor(activeFavorite)" @remove="removeCurrentFavorite" @check="runHealth(activeFavorite.port)" />
          <button v-else-if="selectedListener" type="button" class="text-button add-current" :disabled="!ready || saving" @click="favoriteCurrentPort">{{ saving ? '保存中…' : '☆ 收藏此端口' }}</button>
        </template>
        <template v-if="activeFavorite" #empty>
          <div class="favorite-candidates">
            <h3>监听候选</h3>
            <p v-if="isStale" class="detail-note">扫描完成后可选择当前监听记录。</p>
            <p v-else-if="!favoriteCandidates.length" class="detail-note">当前权限下未发现此端口的 TCP 监听。收藏会继续保留。</p>
            <template v-else>
              <p class="detail-note">选择具体 Socket，查看对应进程详情。</p>
              <button v-for="listener in favoriteCandidates" :key="listenerKey(listener)" type="button" class="candidate-button" @click="chooseCandidate(listenerKey(listener))">{{ listener.processName }} · PID {{ listener.pid }}<span class="mono">{{ listener.address }} · IPv{{ listener.ipVersion ?? '未知' }}</span></button>
            </template>
          </div>
        </template>
      </ListenerDetail>
    </div>
    <footer class="app-footer">仅显示当前权限可见的 TCP LISTEN 快照。</footer>
    <FavoriteEditor v-if="editor" :initial="editor.initial" :editing="editor.editing" :saving="saving" :error="editorError" @save="saveEditor" @remove="removeEditor" @close="editor = null" />
    <TerminateDialog v-if="terminationTarget" :target="terminationTarget" :sending="sending" @cancel="cancelTermination" @confirm="confirmTermination" />
  </main>
</template>

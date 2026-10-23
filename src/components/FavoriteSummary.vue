<script setup lang="ts">
import type { FavoritePort } from "../types/devdock";
import type { HealthState } from "../composables/useHealth";
defineProps<{ favorite: FavoritePort; state: HealthState | undefined; checking: boolean; stale: boolean; count: number; ready: boolean }>();
defineEmits<{ edit: []; check: [] }>();
const labels = { healthy: "HTTP 2xx", http_error: "HTTP 非 2xx", network_error: "连接失败", timeout: "请求超时" };
</script>

<template>
  <div class="favorite-summary">
    <div class="favorite-title"><h2>{{ favorite.label || `端口 ${favorite.port}` }}</h2><button type="button" class="text-button" :disabled="!ready" @click="$emit('edit')">编辑收藏</button></div>
    <p class="favorite-tcp">端口 {{ favorite.port }} · {{ stale ? 'TCP 状态未知' : count ? `${count} 条监听 Socket` : 'TCP 未监听（当前可见范围）' }}</p>
    <template v-if="favorite.healthUrl">
      <p class="health-url mono">{{ favorite.healthUrl }}</p>
      <div class="health-status" role="status" aria-live="polite">
        <span v-if="checking">检测中…</span>
        <template v-else-if="state?.result">
          <strong>{{ labels[state.result.status] }}<template v-if="state.result.statusCode !== null"> · {{ state.result.statusCode }}</template></strong>
          <span>{{ state.result.latencyMs ?? '—' }} ms · {{ new Date(state.result.checkedAt).toLocaleTimeString('zh-CN', { hour12: false }) }}</span>
        </template>
        <span v-else>{{ state?.error || '尚未检测' }}</span>
        <button type="button" class="refresh-button" :disabled="!ready || checking" @click="$emit('check')">{{ checking ? '检测中…' : '检测 HTTP' }}</button>
      </div>
      <p v-if="state?.result?.message" class="detail-note">{{ state.result.message }}</p>
      <p class="detail-note">HTTP 2xx 仅表示此 URL 的响应状态，不等同于业务健康。</p>
    </template>
    <p v-else class="detail-note">未配置健康检查 URL，仅查看 TCP 状态。</p>
  </div>
</template>

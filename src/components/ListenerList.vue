<script setup lang="ts">
import { listenerKey } from "../composables/useListeners";
import type { PortListener } from "../types/devdock";

defineProps<{
  listeners: PortListener[];
  selectedKey: string | null;
  stale: boolean;
  loading: boolean;
  hasSnapshot: boolean;
  failed: boolean;
  searching: boolean;
}>();
defineEmits<{ select: [key: string] }>();
</script>

<template>
  <section class="listener-panel" aria-labelledby="listener-heading" :aria-busy="loading">
    <div class="panel-heading">
      <h2 id="listener-heading">监听 Socket</h2>
      <span class="count">{{ listeners.length }}</span>
    </div>
    <ul v-if="listeners.length" class="listener-list" aria-label="监听记录">
      <li v-for="listener in listeners" :key="listenerKey(listener)">
        <button
          type="button"
          class="listener-row"
          :class="{ selected: selectedKey === listenerKey(listener) }"
          :aria-pressed="selectedKey === listenerKey(listener)"
          :disabled="stale"
          :aria-label="`端口 ${listener.port}，${listener.processName || '不可用'}，PID ${listener.pid}，${listener.address}，IPv${listener.ipVersion ?? '未知'}`"
          @click="$emit('select', listenerKey(listener))"
        >
          <span class="row-top"><span class="port-number">{{ listener.port }}</span><span class="process-name">{{ listener.processName || '不可用' }}</span></span>
          <span class="row-bottom"><span class="socket-address">{{ listener.address }} · {{ listener.ipVersion ? `IPv${listener.ipVersion}` : 'IP 版本不可用' }}</span><span class="pid">PID {{ listener.pid }}</span></span>
        </button>
      </li>
    </ul>
    <div v-else class="list-empty" role="status">
      <template v-if="loading || (!hasSnapshot && !failed)">
        <p>正在扫描端口…</p><span>读取当前权限可见的 TCP 监听。</span>
      </template>
      <template v-else-if="failed"><p>无法确定监听状态</p><span>点击顶部“刷新”重新扫描。</span></template>
      <template v-else-if="searching"><p>没有匹配的监听</p><span>试试其他端口、PID 或进程名。</span></template>
      <template v-else><p>未发现 TCP 监听</p><span>当前权限范围内没有可见记录。</span></template>
    </div>
  </section>
</template>

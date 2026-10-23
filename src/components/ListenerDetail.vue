<script setup lang="ts">
import { computed } from "vue";
import type { PortListener } from "../types/devdock";

const props = defineProps<{ listener: PortListener | null; stale: boolean; loading: boolean; hasListeners: boolean }>();
const startTime = computed(() => {
  const value = props.listener?.processStartTime;
  if (!value) return "不可用";
  const date = new Date(value * 1000);
  return Number.isNaN(date.getTime()) ? "不可用" : date.toLocaleString("zh-CN", { hour12: false });
});
</script>

<template>
  <section class="detail-panel" aria-label="监听详情">
    <slot name="before" />
    <template v-if="listener">
      <div class="detail-title"><h2>端口 <span>{{ listener.port }}</span></h2><span class="listen-badge">TCP LISTEN</span></div>
      <p class="detail-process">{{ listener.processName || '不可用' }}</p>
      <dl class="process-details">
        <div><dt>PID</dt><dd class="mono">{{ listener.pid }}</dd></div>
        <div><dt>监听地址</dt><dd class="mono">{{ listener.address }}</dd></div>
        <div><dt>IP 版本</dt><dd>{{ listener.ipVersion ? `IPv${listener.ipVersion}` : '不可用' }}</dd></div>
        <div><dt>启动时间</dt><dd>{{ startTime }}</dd></div>
        <div class="path-detail"><dt>可执行文件</dt><dd class="mono executable-path">{{ listener.executablePath || '不可用' }}</dd></div>
      </dl>
      <p class="detail-note">TCP 监听仅表示端口正在接收连接，不代表 HTTP 服务健康。</p>
    </template>
    <slot v-else name="empty"><div class="detail-empty">
      <h2>{{ loading ? '正在更新监听信息' : stale ? '等待有效扫描结果' : hasListeners ? '选择一条监听记录' : '当前没有监听记录' }}</h2>
      <p>{{ stale ? '完成扫描后，重新选择进程以查看当前详情。' : hasListeners ? '从左侧选择 Socket，查看真实进程和监听地址。' : '启动本地服务后，点击刷新查看监听。' }}</p>
    </div></slot>
  </section>
</template>

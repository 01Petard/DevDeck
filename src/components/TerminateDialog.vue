<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { TerminationTarget } from "../composables/useTermination";
defineProps<{ target: TerminationTarget; sending: boolean }>();
const emit = defineEmits<{ cancel: []; confirm: [] }>();
const dialog = ref<HTMLDialogElement>();
const cancelButton = ref<HTMLButtonElement>();
onMounted(() => { dialog.value?.showModal(); cancelButton.value?.focus(); });
</script>

<template>
  <dialog ref="dialog" class="favorite-dialog terminate-dialog" aria-labelledby="terminate-title" aria-describedby="terminate-warning" @cancel.prevent="!sending && emit('cancel')">
    <h2 id="terminate-title">结束进程？</h2>
    <p id="terminate-warning">将向 <strong>{{ target.listener.processName || '所选进程' }}</strong> 发送 SIGTERM，可能中断正在进行的本地任务。</p>
    <dl class="process-details">
      <div><dt>PID</dt><dd class="mono">{{ target.listener.pid }}</dd></div>
      <div><dt>所选端口</dt><dd class="mono">{{ target.listener.port }}</dd></div>
      <div><dt>同 PID 已知端口</dt><dd class="mono">{{ target.ports.join('、') }}</dd></div>
    </dl>
    <p class="detail-note">结束进程会同时影响它的其他端口。发送前会重新验证身份与监听；不会升级为强制终止。</p>
    <p v-if="sending" class="detail-note" role="status">正在复核并发送，随后最多等待 2 秒确认退出…</p>
    <div class="dialog-actions">
      <button ref="cancelButton" type="button" class="refresh-button" :disabled="sending" autofocus @click="emit('cancel')">取消</button>
      <button type="button" class="refresh-button danger-button" :disabled="sending" @click="emit('confirm')">{{ sending ? '处理中…' : '发送 SIGTERM' }}</button>
    </div>
  </dialog>
</template>

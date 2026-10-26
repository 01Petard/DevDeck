<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { FavoritePort } from "../types/devdock";
const props = defineProps<{ initial: FavoritePort; editing: boolean; saving: boolean; error: string | null }>();
const emit = defineEmits<{ save: [favorite: FavoritePort]; remove: []; close: [] }>();
const dialog = ref<HTMLDialogElement>();
const port = ref(String(props.initial.port || ""));
const label = ref(props.initial.label);
const note = ref(props.initial.note ?? "");
const healthUrl = ref(props.initial.healthUrl ?? "");
onMounted(() => dialog.value?.showModal());
</script>

<template>
  <dialog ref="dialog" class="favorite-dialog" aria-labelledby="favorite-editor-title" @cancel.prevent="!saving && emit('close')">
    <form @submit.prevent="emit('save', { port: Number(port), label, note, healthUrl: healthUrl || null })">
      <h2 id="favorite-editor-title">{{ editing ? '编辑收藏' : '添加收藏' }}</h2>
      <div class="favorite-fields">
        <label>端口<input v-model="port" type="number" min="1" max="65535" step="1" required :readonly="editing" :disabled="saving" autofocus /></label>
        <label>名称 <span>可选</span><input v-model="label" :disabled="saving" placeholder="例如：本地 API" autocomplete="off" /></label>
      </div>
      <label>备注 <span>可选</span><textarea v-model="note" rows="3" :disabled="saving" placeholder="写下这个端口的用途或说明" :spellcheck="false" /></label>
      <details class="optional-health" :open="!!initial.healthUrl">
        <summary>HTTP 健康检查 <span>可选</span></summary>
        <label>URL<input v-model="healthUrl" :disabled="saving" type="text" inputmode="url" placeholder="http://127.0.0.1:8080/health" :spellcheck="false" autocomplete="off" /></label>
        <p class="detail-note">仅回环 HTTP(S) 地址，端口须与收藏一致。留空则不检测 HTTP。</p>
      </details>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-actions">
        <button v-if="editing" type="button" class="text-button remove-favorite" :disabled="saving" @click="emit('remove')">取消收藏</button>
        <button type="button" class="refresh-button" :disabled="saving" @click="emit('close')">取消</button>
        <button type="submit" class="refresh-button primary-button" :disabled="saving">{{ saving ? '保存中…' : '保存' }}</button>
      </div>
    </form>
  </dialog>
</template>

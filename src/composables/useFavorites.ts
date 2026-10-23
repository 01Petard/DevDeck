import { ref, shallowRef } from "vue";
import { createSettingsService, validateFavorite } from "../services/settings.ts";
import type { FavoritePort } from "../types/devdock.ts";

export function useFavorites(service = createSettingsService()) {
  const favorites = shallowRef<FavoritePort[]>([]);
  const ready = ref(false);
  const loading = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);
  async function read(retry = false) {
    if (loading.value || saving.value) return;
    loading.value = true;
    ready.value = false;
    error.value = null;
    try { favorites.value = (await service.read(retry)).favorites.sort((a, b) => a.port - b.port); ready.value = true; }
    catch (cause) { error.value = cause instanceof Error ? cause.message : "收藏配置无法读取。"; }
    finally { loading.value = false; }
  }
  async function save(favorite: FavoritePort, originalPort: number | null) {
    if (!ready.value || saving.value) throw new Error("收藏配置暂不可写，请稍后重试。");
    const validated = validateFavorite(favorite);
    const next = [...favorites.value.filter(item => item.port !== originalPort && item.port !== validated.port), validated].sort((a, b) => a.port - b.port);
    saving.value = true;
    try { await service.save({ schemaVersion: 1, favorites: next }); favorites.value = next; }
    finally { saving.value = false; }
  }
  async function remove(port: number) {
    if (!ready.value || saving.value) throw new Error("收藏配置暂不可写。");
    const next = favorites.value.filter(item => item.port !== port);
    saving.value = true;
    try { await service.save({ schemaVersion: 1, favorites: next }); favorites.value = next; }
    finally { saving.value = false; }
  }
  return { favorites, ready, loading, saving, error, read, save, remove };
}

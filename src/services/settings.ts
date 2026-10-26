import { load } from "@tauri-apps/plugin-store";
import type { DevDockSettings, FavoritePort } from "../types/devdock.ts";

export function validateFavorite(input: FavoritePort): FavoritePort {
  if (!Number.isInteger(input.port) || input.port < 1 || input.port > 65535) throw new Error("端口须为 1–65535 的整数。");
  const label = input.label.trim();
  if ([...label].length > 60) throw new Error("别名最多 60 个字符。");
  const note = input.note?.trim() ?? "";
  if ([...note].length > 500) throw new Error("备注说明最多 500 个字符。");
  const healthUrl = input.healthUrl?.trim() || null;
  if (healthUrl !== null) {
    let url: URL;
    try { url = new URL(healthUrl); } catch { throw new Error("请输入完整的回环 HTTP(S) URL。"); }
    const authority = healthUrl.split("://")[1]?.split(/[/?#]/)[0] ?? "";
    if ([...healthUrl].length > 2048 || /[\s\u0000-\u001f\u007f\\]/u.test(healthUrl)
      || !/^(localhost|127\.0\.0\.1|\[::1\])(?::[0-9]+)?$/i.test(authority)
      || !["http:", "https:"].includes(url.protocol) || url.username || url.password || healthUrl.includes("#")
      || !["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)) {
      throw new Error("仅支持 localhost、127.0.0.1 或 [::1]，且不能含用户信息或片段。");
    }
    const effectivePort = Number(url.port || (url.protocol === "https:" ? 443 : 80));
    if (effectivePort !== input.port) throw new Error("健康检查 URL 的端口必须与收藏端口相同。");
  }
  return { port: input.port, label, healthUrl, ...(note ? { note } : {}) };
}

export function parseSettings(raw: unknown): DevDockSettings {
  if (!raw || typeof raw !== "object" || !("schemaVersion" in raw) || raw.schemaVersion !== 1
    || !("favorites" in raw) || !Array.isArray(raw.favorites)) throw new Error("配置格式或版本不支持。");
  const ports = new Set<number>();
  const favorites = raw.favorites.map((item: unknown) => {
    if (!item || typeof item !== "object" || !("port" in item) || typeof item.port !== "number"
      || !("label" in item) || typeof item.label !== "string"
      || !("healthUrl" in item) || (item.healthUrl !== null && typeof item.healthUrl !== "string")
      || ("note" in item && typeof item.note !== "string")) throw new Error("收藏字段无效。");
    const favorite = validateFavorite(item as FavoritePort);
    if (favorite.label !== item.label || favorite.healthUrl !== item.healthUrl
      || (favorite.note ?? "") !== ("note" in item ? item.note : "") || ports.has(favorite.port)) throw new Error("收藏内容无效或端口重复。");
    ports.add(favorite.port);
    return favorite;
  });
  return { schemaVersion: 1, favorites };
}

export interface SettingsStore {
  get<T>(key: string): Promise<T | undefined>;
  set(key: string, value: unknown): Promise<void>;
  save(): Promise<void>;
  reload(options: { ignoreDefaults: boolean }): Promise<void>;
  close(): Promise<void>;
}

export function createSettingsService(open: () => Promise<SettingsStore> = () => load("devdock.json", { autoSave: false })) {
  let store: SettingsStore | null = null;
  let current: DevDockSettings | null = null;
  let writable = false;
  return {
    async read(retry = false): Promise<DevDockSettings> {
      writable = false;
      try {
        store ??= await open();
        if (retry) await store.reload({ ignoreDefaults: true });
        if (await store.get("__loadError") !== undefined) throw new Error();
        const raw = await store.get("settings");
        current = raw === undefined ? { schemaVersion: 1, favorites: [] } : parseSettings(raw);
        writable = true;
        return current;
      } catch { throw new Error("收藏配置无法读取或格式不受支持。原文件未覆盖，请修复 devdock.json 后重新读取。"); }
    },
    async save(next: DevDockSettings): Promise<void> {
      if (!writable || !store || !current) throw new Error("请先恢复有效配置，再保存收藏。");
      const validated = parseSettings(next);
      try {
        await store.set("settings", validated);
        await store.save();
        current = validated;
      } catch {
        try { await store.set("settings", current); }
        catch {
          writable = false;
          await store.close().catch(() => {});
          store = null;
        }
        throw new Error("收藏保存失败，修改尚未保存。请检查文件权限后重试。");
      }
    },
  };
}

import assert from "node:assert/strict";
import test from "node:test";
import { createSettingsService, parseSettings, validateFavorite } from "../src/services/settings.ts";
import type { SettingsStore } from "../src/services/settings.ts";
import { useFavorites } from "../src/composables/useFavorites.ts";
import { useHealth } from "../src/composables/useHealth.ts";
import type { DevDockSettings, FavoritePort, HealthCheckResult } from "../src/types/devdock.ts";

const favorite: FavoritePort = { port: 8080, label: "API", healthUrl: "http://localhost:8080/health" };
const settings: DevDockSettings = { schemaVersion: 1, favorites: [favorite] };

test("optional multiline notes survive persistence while older favorites remain compatible", async () => {
  const withNote = { ...favorite, note: "  本地联调 API\n由开发环境启动。  " };
  const validated = validateFavorite(withNote);
  assert.equal(validated.note, "本地联调 API\n由开发环境启动。");
  assert.deepEqual(parseSettings(settings), settings);
  assert.deepEqual(parseSettings({ ...settings, favorites: [validated] }).favorites[0], validated);
  assert.equal(validateFavorite({ ...favorite, note: "😀".repeat(500) }).note?.length, 1000);
  assert.throws(() => validateFavorite({ ...favorite, note: "😀".repeat(501) }));
  for (const note of [null, 123, []]) assert.throws(() => parseSettings({ ...settings, favorites: [{ ...favorite, note }] }));
  assert.equal(validateFavorite({ ...favorite, note: "  " }).note, undefined);
  const io = testStore();
  const state = useFavorites(createSettingsService(async () => io.store));
  await state.read();
  await state.save(validated, 8080);
  const restored = await createSettingsService(async () => io.store).read();
  assert.equal(restored.favorites[0]?.note, "本地联调 API\n由开发环境启动。");
});

test("favorite validation trims input, counts Unicode and checks effective URL port", () => {
  assert.deepEqual(validateFavorite({ port: 5432, label: " DB ", healthUrl: " " }), { port: 5432, label: "DB", healthUrl: null });
  assert.equal(validateFavorite({ port: 443, label: "😀".repeat(60), healthUrl: "https://localhost/" }).port, 443);
  assert.equal(validateFavorite({ port: 80, label: "", healthUrl: "http://[::1]/" }).port, 80);
  for (const change of [{ port: 0 }, { port: 65536 }, { port: 1.5 }, { port: NaN }, { label: "😀".repeat(61) },
    { healthUrl: "http://localhost:8081/" }, { healthUrl: "https://localhost" },
    ...["example.com", "localhost.evil.com", "localhost.", "127.0.0.2", "127.1", "2130706433", "0x7f000001", "%6cocalhost", "user:pass@localhost", "@localhost", "[::ffff:127.0.0.1]"].map(host => ({ healthUrl: `http://${host}:8080/` })),
    { healthUrl: "http://localhost:8080/#" }, { healthUrl: "ftp://localhost:8080/" }, { healthUrl: "http://local\nhost:8080/" },
    { healthUrl: `http://localhost:8080/${"x".repeat(2048)}` },
  ]) assert.throws(() => validateFavorite({ ...favorite, ...change }), JSON.stringify(change));
});

test("persisted schema rejects damaged fields, newer versions and duplicate ports", () => {
  assert.deepEqual(parseSettings(settings), settings);
  for (const raw of [null, [], {}, { schemaVersion: 2, favorites: [] }, { schemaVersion: 1, favorites: {} },
    { schemaVersion: 1, favorites: [favorite, favorite] },
    ...[{ port: "8080" }, { label: 1 }, { healthUrl: undefined }, { healthUrl: false }, { label: " API " }, { healthUrl: " " }].map(change => ({ schemaVersion: 1, favorites: [{ ...favorite, ...change }] })),
  ]) assert.throws(() => parseSettings(raw));
});

function testStore(initial: unknown = settings) {
  const values = new Map<string, unknown>([["settings", structuredClone(initial)]]);
  let writes = 0;
  let failure = false;
  let failRollback = false;
  let closed = false;
  let disk = structuredClone(initial);
  const store: SettingsStore = {
    async get<T>(key: string) { return values.get(key) as T | undefined; },
    async set(key, value) { if (failure && failRollback && writes > 0) throw Error("private token"); writes++; values.set(key, structuredClone(value)); },
    async save() { if (failure) throw Error("private token"); disk = structuredClone(values.get("settings")); },
    async reload() { values.delete("__loadError"); values.set("settings", structuredClone(disk)); },
    async close() { closed = true; },
  };
  return { store, values, writes: () => writes, disk: () => disk, closed: () => closed,
    fail: (rollback = false) => { failure = true; failRollback = rollback; }, recover: () => { failure = false; }, repair: () => { disk = settings; } };
}

test("damaged settings block writes, preserve original and allow explicit reload after repair", async () => {
  const io = testStore({ schemaVersion: 2, favorites: [] });
  io.values.set("__loadError", true);
  const service = createSettingsService(async () => io.store);
  await assert.rejects(service.read(), /原文件未覆盖/);
  await assert.rejects(service.save(settings));
  assert.equal(io.writes(), 0);
  assert.deepEqual(io.disk(), { schemaVersion: 2, favorites: [] });
  io.repair();
  assert.deepEqual(await service.read(true), settings);
  await service.save({ schemaVersion: 1, favorites: [] });
  assert.deepEqual(io.disk(), { schemaVersion: 1, favorites: [] });
});

test("save failure rolls back plugin cache, preserves UI and draft, then permits retry", async () => {
  const io = testStore();
  const state = useFavorites(createSettingsService(async () => io.store));
  await state.read();
  io.fail();
  await assert.rejects(state.save({ ...favorite, label: "draft" }, 8080), /保存失败/);
  assert.equal(state.favorites.value[0]?.label, "API");
  assert.deepEqual(io.values.get("settings"), settings);
  assert.deepEqual(io.disk(), settings);
  assert.equal(state.saving.value, false);
  io.recover();
  await state.save({ ...favorite, label: "draft" }, 8080);
  assert.equal(state.favorites.value[0]?.label, "draft");
});

test("unrecoverable IPC rollback closes Store to prevent exit from writing unsaved draft", async () => {
  const io = testStore();
  const service = createSettingsService(async () => io.store);
  await service.read();
  io.fail(true);
  await assert.rejects(service.save({ schemaVersion: 1, favorites: [] }), /保存失败/);
  assert.equal(io.closed(), true);
  await assert.rejects(service.save(settings), /恢复有效配置/);
});

test("favorites are port unique and persist without requiring an active listener", async () => {
  const io = testStore();
  const state = useFavorites(createSettingsService(async () => io.store));
  await state.read();
  await state.save({ port: 5432, label: " DB ", healthUrl: null }, null);
  await state.save({ ...favorite, label: "new alias" }, null);
  assert.deepEqual(state.favorites.value.map(f => f.port), [5432, 8080]);
  assert.equal(state.favorites.value[1]?.label, "new alias");
  const restarted = createSettingsService(async () => io.store);
  assert.equal((await restarted.read()).favorites.length, 2);
  await state.remove(8080);
  assert.deepEqual(state.favorites.value.map(f => f.port), [5432]);
});

const healthy = (url: string): HealthCheckResult => ({ url, status: "healthy", statusCode: 200, latencyMs: 5, checkedAt: 1000, message: null });
const tick = () => new Promise<void>(resolve => setImmediate(resolve));
test("health sends no automatic or unconfigured requests, limits concurrency to four and deduplicates", async () => {
  const favorites = Array.from({ length: 8 }, (_, i) => ({ port: 8000 + i, label: "", healthUrl: i === 7 ? null : `http://localhost:${8000 + i}/` }));
  const pending: Array<() => void> = [];
  let calls = 0;
  const state = useHealth(() => favorites, url => { calls++; return new Promise(resolve => pending.push(() => resolve(healthy(url)))); });
  assert.equal(calls, 0);
  for (const f of favorites) { state.run(f.port); state.run(f.port); }
  state.run(9999);
  assert.equal(calls, 4);
  pending.splice(0).forEach(finish => finish());
  await tick();
  assert.equal(calls, 7);
  pending.splice(0).forEach(finish => finish());
  await tick();
  assert.equal(Object.values(state.states.value).filter(s => s.result?.status === "healthy").length, 7);
});

test("edited or removed favorites ignore obsolete checks and queued jobs", async () => {
  let favorites = Array.from({ length: 5 }, (_, i) => ({ ...favorite, port: 8000 + i, healthUrl: `http://localhost:${8000 + i}/` }));
  const pending: Array<() => void> = [];
  let calls = 0;
  const state = useHealth(() => favorites, url => { calls++; return new Promise(resolve => pending.push(() => resolve(healthy(url)))); });
  favorites.forEach(f => state.run(f.port));
  favorites = [];
  state.invalidate(8000);
  pending.splice(0).forEach(f => f());
  await tick();
  assert.equal(calls, 4);
  assert.equal(state.states.value[8000], undefined);
  assert.equal(state.states.value[8004], undefined);
});

test("editing a URL keeps the port locked until the old request finishes", async () => {
  let favorites = [favorite];
  let finish: () => void = () => {};
  let calls = 0;
  const state = useHealth(() => favorites, url => { calls++; return new Promise(resolve => { finish = () => resolve(healthy(url)); }); });
  state.run(8080);
  favorites = [{ ...favorite, healthUrl: "http://localhost:8080/new" }];
  state.invalidate(8080);
  state.run(8080);
  assert.equal(calls, 1);
  assert.equal(state.busyPorts.value.has(8080), true);
  finish(); await tick();
  assert.equal(state.states.value[8080], undefined);
  assert.equal(state.busyPorts.value.has(8080), false);
  state.run(8080);
  assert.equal(calls, 2);
  finish(); await tick();
  assert.equal(state.states.value[8080]?.result?.url, favorites[0]?.healthUrl);
});

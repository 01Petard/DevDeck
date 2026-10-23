import assert from "node:assert/strict";
import test from "node:test";
import { listenerKey, useListeners } from "../src/composables/useListeners.ts";
import { normalizeScanError } from "../src/services/native.ts";
import type { PortListener } from "../src/types/devdock.ts";

const node: PortListener = {
  pid: 202, processName: "Node", processStartTime: 100,
  executablePath: "/test/node", protocol: "TCP", address: "::1", port: 5173, ipVersion: 6,
};
const java: PortListener = {
  pid: 101, processName: "java", processStartTime: null,
  executablePath: null, protocol: "TCP", address: "*", port: 8080, ipVersion: 4,
};

test("search matches port, PID and case-insensitive process name while keeping port order", async () => {
  const state = useListeners(async () => [java, node], () => 10_000);
  await state.refresh();
  assert.deepEqual(state.filteredListeners.value.map(l => l.port), [5173, 8080]);
  for (const [query, ports] of [[" 517 ", [5173]], ["202", [5173]], ["nOdE", [5173]], ["java", [8080]], ["no-match", []]] as const) {
    state.query.value = query;
    assert.deepEqual(state.filteredListeners.value.map(l => l.port), ports);
  }
});

test("socket identity includes process start time, address and IP version", () => {
  for (const change of [{ processStartTime: 101 }, { address: "*" }, { ipVersion: 4 as const }]) {
    assert.notEqual(listenerKey(node), listenerKey({ ...node, ...change }));
  }
});

test("empty successful snapshot differs from a failed scan with old data", async () => {
  let clock = 10_000;
  let fail = false;
  const state = useListeners(async () => {
    if (fail) throw { code: "SCAN_FAILED", message: "/private/secret" };
    return [node];
  }, () => clock);
  await state.refresh();
  state.selectListener(listenerKey(node));
  assert.equal(state.selectedListener.value?.pid, 202);
  clock += 1000;
  fail = true;
  await state.refresh();
  assert.deepEqual(state.listeners.value, [node]);
  assert.equal(state.lastScanAt.value, 10_000);
  assert.equal(state.error.value?.code, "SCAN_FAILED");
  assert.equal(state.isStale.value, true);
  assert.equal(state.selectedListener.value, null);
  state.selectListener(listenerKey(node));
  assert.equal(state.selectedListener.value, null);
  assert.ok(!state.error.value?.message.includes("secret"));

  const empty = useListeners(async () => [], () => clock);
  await empty.refresh();
  assert.equal(empty.error.value, null);
  assert.equal(empty.isStale.value, false);
  assert.equal(empty.lastScanAt.value, clock);
  assert.deepEqual(empty.listeners.value, []);
});

test("refresh prevents concurrent scans and never exposes an old actionable selection", async () => {
  let clock = 10_000;
  let calls = 0;
  let finish: (value: PortListener[]) => void = () => {};
  const state = useListeners(() => {
    calls++;
    return calls === 1 ? Promise.resolve([node]) : new Promise(resolve => { finish = resolve; });
  }, () => clock);
  await state.refresh();
  state.selectListener(listenerKey(node));
  clock += 1000;
  const refreshing = state.refresh();
  assert.equal(state.isRefreshing.value, true);
  assert.equal(state.isStale.value, true);
  assert.equal(state.selectedListener.value, null);
  await state.refresh();
  assert.equal(calls, 2);
  finish([]);
  await refreshing;
  assert.equal(state.selectedListener.value, null);
  assert.equal(state.selectedKey.value, null);
  assert.equal(state.isRefreshing.value, false);
});

test("refresh keeps selection only when process and socket identity still match", async () => {
  let clock = 10_000;
  let rows = [node];
  const state = useListeners(async () => rows, () => clock);
  await state.refresh();
  state.selectListener(listenerKey(node));
  clock += 1000;
  await state.refresh();
  assert.equal(state.selectedListener.value?.pid, 202);
  rows = [{ ...node, processStartTime: 999 }];
  clock += 1000;
  await state.refresh();
  assert.equal(state.selectedKey.value, null);
  assert.equal(state.selectedListener.value, null);
});

test("focus refresh is throttled for two seconds and manual clicks are debounced", async () => {
  let clock = 10_000;
  let calls = 0;
  const state = useListeners(async () => { calls++; return []; }, () => clock);
  await state.refresh();
  clock += 100;
  await state.refresh();
  assert.equal(calls, 1);
  clock = 11_999;
  await state.refresh("focus");
  assert.equal(calls, 1);
  clock = 12_000;
  await state.refresh("focus");
  assert.equal(calls, 2);
});

test("search clears selection and unknown IPC failures cannot expose raw details", async () => {
  const state = useListeners(async () => [node], () => 10_000);
  await state.refresh();
  state.selectListener(listenerKey(node));
  state.query.value = "java";
  assert.equal(state.selectedListener.value, null);
  assert.equal(state.selectedKey.value, null);
  assert.equal(normalizeScanError({ code: "SCAN_TIMEOUT" }).code, "SCAN_TIMEOUT");
  const unknown = normalizeScanError("secret token and /private/path");
  assert.equal(unknown.code, "INTERNAL_ERROR");
  assert.ok(!unknown.message.includes("secret"));
});

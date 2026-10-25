import assert from "node:assert/strict";
import test from "node:test";
import { ref } from "vue";
import { useTermination } from "../src/composables/useTermination.ts";
import { normalizeTerminateError } from "../src/services/native.ts";
import { useListeners } from "../src/composables/useListeners.ts";
import type { PortListener, TerminateRequest } from "../src/types/devdock.ts";

const listener: PortListener = { pid:202, processName:"Python", processStartTime:100, executablePath:"/test/python", protocol:"TCP", address:"127.0.0.1", port:8080, ipVersion:4 };

test("confirmation is explicit, includes all known same-PID ports and cancellation sends nothing", async () => {
  let calls = 0;
  const state = useTermination(() => listener, () => [listener,{ ...listener,port:8081 },{ ...listener,address:"::1" },{ ...listener,pid:303,port:9999 }], () => false, async () => {}, async () => { calls++; return { signalSent:true,processExited:true,message:"" }; });
  await state.confirm();
  assert.equal(calls,0);
  state.open();
  assert.deepEqual(state.target.value?.ports,[8080,8081]);
  assert.equal(calls,0);
  state.cancel();
  await state.confirm();
  assert.equal(state.target.value,null);
  assert.equal(calls,0);
});

test("missing identity, protected PID, absent selection and stale snapshots block opening", () => {
  for (const [selected, stale] of [[null,false],[listener,true],[{...listener,processStartTime:null},false],
    [{...listener,processStartTime:0},false],[{...listener,pid:1},false],[{...listener,pid:4294967295},false]] as const) {
    const state = useTermination(() => selected, () => [listener], () => stale, async () => {});
    assert.ok(state.disabledReason.value);
    state.open();
    assert.equal(state.target.value,null);
  }
});

test("changed socket, process identity or stale scan after opening never reaches IPC", async () => {
  for (const change of [null,{...listener,processStartTime:101},{...listener,address:"::1"},listener]) {
    const current = ref<PortListener | null>(listener);
    const stale = ref(false);
    let calls = 0;
    const state = useTermination(() => current.value, () => [listener], () => stale.value, async () => {}, async () => { calls++; throw Error(); });
    state.open();
    current.value = change;
    if (change === listener) stale.value = true;
    await state.confirm();
    assert.equal(calls,0);
    assert.equal(state.target.value,null);
    assert.match(state.notice.value?.message ?? "",/未发送/);
  }
});

test("confirmation submits exact identity once, blocks dismissal while sending and refreshes afterwards", async () => {
  let finish: () => void = () => {};
  const requests: TerminateRequest[] = [];
  let refreshes = 0;
  const state = useTermination(() => listener, () => [listener], () => false, async () => { refreshes++; }, async request => {
    requests.push(request);
    return new Promise(resolve => { finish = () => resolve({ signalSent:true,processExited:true,message:"private details" }); });
  });
  state.open();
  const pending = state.confirm();
  assert.equal(state.sending.value,true);
  state.cancel();
  assert.ok(state.target.value);
  await state.confirm();
  assert.deepEqual(requests,[{pid:202,processStartTime:100,port:8080}]);
  finish(); await pending;
  assert.equal(refreshes,1);
  assert.equal(state.target.value,null);
  assert.equal(state.sending.value,false);
  assert.equal(state.notice.value?.kind,"exited");
  assert.ok(!state.notice.value?.message.includes("private"));
});

test("signal sent without confirmed exit is never shown as exited", async () => {
  const state = useTermination(() => listener, () => [listener], () => false, async () => {}, async () => ({ signalSent:true,processExited:false,message:"done" }));
  state.open(); await state.confirm();
  assert.equal(state.notice.value?.kind,"signal_sent");
  assert.match(state.notice.value?.message ?? "",/尚未确认原进程退出/);
});

test("backend rejection refreshes and errors are redacted without falsely claiming unknown signal outcome", async () => {
  let refreshes = 0;
  const state = useTermination(() => listener, () => [listener], () => false, async () => { refreshes++; }, async () => { throw {code:"PROCESS_IDENTITY_CHANGED",message:"secret /private/path"}; });
  state.open(); await state.confirm();
  assert.equal(state.notice.value?.kind,"failed");
  assert.equal(refreshes,1);
  assert.ok(!state.notice.value?.message.includes("secret"));
  for (const code of ["INVALID_ARGUMENT","PROCESS_NOT_FOUND","PROCESS_UNVERIFIABLE","PROCESS_PROTECTED","PERMISSION_DENIED","SIGNAL_FAILED"]) {
    const error = normalizeTerminateError({code,message:"secret"});
    assert.equal(error.code,code);
    assert.ok(!error.message.includes("secret"));
  }
  const unknown = normalizeTerminateError("secret");
  assert.equal(unknown.code,"INTERNAL_ERROR");
  assert.match(unknown.message,/无法确认操作结果/);
  assert.ok(!unknown.message.includes("未发送"));
});

test("post-action refresh bypasses manual debounce and clears vanished selection", async () => {
  let rows = [listener];
  let calls = 0;
  const state = useListeners(async () => { calls++; return rows; }, () => 1000);
  await state.refresh();
  rows = [];
  await state.refresh();
  assert.equal(calls,1);
  await state.refresh("action");
  assert.equal(calls,2);
  assert.deepEqual(state.listeners.value,[]);
});

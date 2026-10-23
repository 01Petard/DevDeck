import { shallowRef } from "vue";
import { checkHttpHealth } from "../services/native.ts";
import type { FavoritePort, HealthCheckResult } from "../types/devdock.ts";

export interface HealthState { url: string; checking: boolean; result: HealthCheckResult | null; error: string | null }

export function useHealth(getFavorites: () => FavoritePort[], check = checkHttpHealth) {
  const states = shallowRef<Record<number, HealthState>>({});
  const busyPorts = shallowRef(new Set<number>());
  const queue: Array<{ port: number; url: string; state: HealthState }> = [];
  let active = 0;
  function busy(port: number, value: boolean) {
    const next = new Set(busyPorts.value);
    if (value) next.add(port); else next.delete(port);
    busyPorts.value = next;
  }
  function set(port: number, state: HealthState) { states.value = { ...states.value, [port]: state }; }
  function configured(port: number, url: string) { return getFavorites().some(item => item.port === port && item.healthUrl === url); }
  function drain() {
    while (active < 4 && queue.length) {
      const job = queue.shift()!;
      if (states.value[job.port] !== job.state) { busy(job.port, false); continue; }
      if (!configured(job.port, job.url)) { invalidate(job.port); busy(job.port, false); continue; }
      active++;
      void (async () => {
        let result: HealthCheckResult | null = null;
        let error: string | null = null;
        try { result = await check(job.url); }
        catch { error = "无法执行检测，请检查 URL 或稍后重试。"; }
        if (states.value[job.port] === job.state) {
          if (configured(job.port, job.url)) set(job.port, { url: job.url, checking: false, result, error });
          else invalidate(job.port);
        }
        busy(job.port, false);
        active--;
        drain();
      })();
    }
  }
  function run(port: number) {
    const url = getFavorites().find(item => item.port === port)?.healthUrl;
    if (!url || busyPorts.value.has(port)) return;
    const state = { url, checking: true, result: null, error: null };
    set(port, state);
    busy(port, true);
    queue.push({ port, url, state });
    drain();
  }
  function invalidate(port: number) {
    const next = { ...states.value };
    delete next[port];
    states.value = next;
  }
  return { states, busyPorts, run, invalidate };
}

import { invoke } from "@tauri-apps/api/core";
import type { PortListener } from "../types/devdock";

export function listListeners(): Promise<PortListener[]> {
  return invoke<PortListener[]>("list_listeners");
}

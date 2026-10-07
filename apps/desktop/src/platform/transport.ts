import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DownloadTransport } from "@video/sdk";
import type { Job } from "@video/contracts";
export const native = isTauri();
export const transport: DownloadTransport = {
  inspect: (source) => invoke("inspect_source", { source }),
  create: (options) => invoke("create_download", { options }),
  jobs: () => invoke("list_downloads"),
  settings: () => invoke("get_settings"),
  runtime: () => invoke("runtime_status"),
  action: (command, args) => invoke(command, args),
  subscribe: async (callback) =>
    listen<Job>("download-update", (e) => callback(e.payload)),
};

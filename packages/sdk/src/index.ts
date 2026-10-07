import type {
  DownloadOptions,
  Inspection,
  Job,
  Settings,
  RuntimeStatus,
} from "@video/contracts";
export interface DownloadTransport {
  inspect(source: string): Promise<Inspection>;
  create(options: DownloadOptions): Promise<Job>;
  jobs(): Promise<Job[]>;
  settings(): Promise<Settings>;
  runtime(): Promise<RuntimeStatus>;
  action(command: string, args?: Record<string, unknown>): Promise<void>;
  subscribe(callback: (job: Job) => void): Promise<() => void>;
}

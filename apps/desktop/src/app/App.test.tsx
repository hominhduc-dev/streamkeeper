// @vitest-environment jsdom
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import type { Job, Inspection, Settings } from "@video/contracts";
const mocks = vi.hoisted(() => ({
  jobs: vi.fn(),
  settings: vi.fn(),
  runtime: vi.fn(),
  inspect: vi.fn(),
  create: vi.fn(),
  action: vi.fn(),
  subscribe: vi.fn(),
}));
vi.mock("../platform/transport", () => ({ native: true, transport: mocks }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));
import App from "./App";
const settings: Settings = {
  preventSleep: true,
  outputDir: "C:\\Videos",
  cacheDir: "C:\\Cache",
  concurrency: 8,
  keepCache: false,
  theme: "dark",
};
const inspection: Inspection = {
  id: "inspect",
  title: "Video mẫu",
  provider: "HLS",
  duration: 12,
  videoTracks: [
    {
      id: "v0",
      kind: "video",
      label: "1920x1080",
      language: null,
      bandwidth: 1000000,
      resolution: "1920x1080",
      codecs: "avc1",
      audioGroup: "a",
    },
  ],
  audioTracks: [
    {
      id: "audio0",
      kind: "audio",
      label: "Tiếng Anh",
      language: "eng",
      bandwidth: null,
      resolution: null,
      codecs: null,
      audioGroup: "a",
    },
  ],
  subtitleTracks: [
    {
      id: "subtitle0",
      kind: "subtitle",
      label: "Tiếng Việt",
      language: "vie",
      bandwidth: null,
      resolution: null,
      codecs: null,
      audioGroup: "sub",
    },
  ],
};
function job(state: string): Job {
  return {
    id: "job",
    title: "Video kiểm thử",
    provider: "HLS",
    state,
    stage: state,
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
    downloadedBytes: 100,
    estimatedTotalBytes: 1000,
    completedSegments: 1,
    totalSegments: 10,
    speed: 10,
    eta: 90,
    duration: 12,
    output: "C:\\Videos\\test.mp4",
    error: state === "failed" ? "Mất kết nối" : null,
    errorCode: null,
    options: {
      inspectionId: "inspect",
      videoId: "v0",
      audioId: null,
      subtitleId: null,
      subtitleMode: "none",
      subtitleOffset: 0,
      outputDir: settings.outputDir,
      filename: "test",
      format: "mp4",
    },
  };
}
function mount(route = "/") {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <MemoryRouter initialEntries={[route]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
beforeEach(() => {
  vi.resetAllMocks();
  mocks.jobs.mockResolvedValue([]);
  mocks.settings.mockResolvedValue(settings);
  mocks.runtime.mockResolvedValue({
    ffmpeg: true,
    ffprobe: true,
    ffmpegVersion: "fixture",
    dataDir: "C:\\Data",
  });
  mocks.subscribe.mockResolvedValue(() => {});
  mocks.action.mockResolvedValue(undefined);
  mocks.inspect.mockResolvedValue(inspection);
  mocks.create.mockResolvedValue(job("queued"));
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: () => ({
      matches: true,
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  });
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute("open");
  };
});
afterEach(cleanup);
describe("desktop user flows", () => {
  it("renders empty downloads", async () => {
    mount();
    await screen.findByText("Video tiếp theo bắt đầu từ một đường dẫn");
    expect(screen.queryByText("Video kiểm thử")).toBeNull();
  });
  it("opens completed output from history", async () => {
    mocks.jobs.mockResolvedValue([job("completed")]);
    mount("/history");
    fireEvent.click(await screen.findByRole("button", { name: "Mở video" }));
    await waitFor(() =>
      expect(mocks.action).toHaveBeenCalledWith("open_artifact", {
        id: "job",
        folder: false,
      }),
    );
  });
  it("persists settings", async () => {
    mount("/settings");
    const field = await screen.findByLabelText("Số request song song");
    fireEvent.change(field, { target: { value: "4" } });
    fireEvent.click(
      screen.getByRole("checkbox", { name: /Giữ máy hoạt động/ }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Lưu cài đặt" }));
    await waitFor(() =>
      expect(mocks.action).toHaveBeenCalledWith("update_settings", {
        settings: { ...settings, concurrency: 4, preventSleep: false },
      }),
    );
  });
  it("creates job with Vietnamese subtitles", async () => {
    mount();
    await screen.findByText("Video tiếp theo bắt đầu từ một đường dẫn");
    fireEvent.click(screen.getByRole("button", { name: "Thêm video" }));
    fireEvent.change(await screen.findByLabelText("Đường dẫn video"), {
      target: { value: "https://example.com/master.m3u8" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Phân tích" }));
    await screen.findByText("Video mẫu");
    fireEvent.change(screen.getByLabelText("Phụ đề"), {
      target: { value: "subtitle0" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Thêm vào hàng đợi" }));
    await waitFor(() =>
      expect(mocks.create).toHaveBeenCalledWith(
        expect.objectContaining({
          videoId: "v0",
          audioId: "audio0",
          subtitleId: "subtitle0",
          subtitleMode: "soft",
        }),
      ),
    );
  });
  it("allows retry after inspection error", async () => {
    mocks.inspect.mockRejectedValueOnce("Nguồn không được hỗ trợ");
    mount();
    await screen.findByText("Video tiếp theo bắt đầu từ một đường dẫn");
    fireEvent.click(screen.getByRole("button", { name: "Thêm video" }));
    fireEvent.change(await screen.findByLabelText("Đường dẫn video"), {
      target: { value: "https://example.com/x" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Phân tích" }));
    await screen.findByText("Nguồn không được hỗ trợ");
    fireEvent.click(screen.getByRole("button", { name: "Phân tích" }));
    await screen.findByText("Video mẫu");
  });
  it("pauses and resumes", async () => {
    mocks.jobs.mockResolvedValue([job("downloading")]);
    const view = mount();
    fireEvent.click(await screen.findByRole("button", { name: "Tạm dừng" }));
    await waitFor(() =>
      expect(mocks.action).toHaveBeenCalledWith("pause_download", {
        id: "job",
      }),
    );
    view.unmount();
    mocks.jobs.mockResolvedValue([job("paused")]);
    mount();
    fireEvent.click(await screen.findByRole("button", { name: "Tiếp tục" }));
    await waitFor(() =>
      expect(mocks.action).toHaveBeenCalledWith("resume_download", {
        id: "job",
      }),
    );
  });
  it("cancels with cache cleanup", async () => {
    mocks.jobs.mockResolvedValue([job("queued")]);
    mount();
    fireEvent.click(await screen.findByRole("button", { name: "Hủy tác vụ" }));
    fireEvent.click(screen.getByRole("button", { name: "Hủy và dọn cache" }));
    await waitFor(() =>
      expect(mocks.action).toHaveBeenCalledWith("cancel_download", {
        id: "job",
        keepCache: false,
      }),
    );
  });
  it("shows detail and recoverable error", async () => {
    mocks.jobs.mockResolvedValue([job("failed")]);
    mount();
    fireEvent.click(
      await screen.findByRole("button", { name: "Video kiểm thử" }),
    );
    expect(
      screen.getByRole("dialog", { name: "Chi tiết tác vụ" }),
    ).toBeTruthy();
    expect(screen.getAllByText("Mất kết nối").length).toBeGreaterThan(0);
  });
  it("reports native action failure", async () => {
    mocks.jobs.mockResolvedValue([job("completed")]);
    mocks.action.mockRejectedValueOnce("File đã bị xóa");
    mount("/history");
    fireEvent.click(await screen.findByRole("button", { name: "Mở video" }));
    await screen.findByText("File đã bị xóa");
  });
});

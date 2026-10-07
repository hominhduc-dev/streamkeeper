// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Movie } from "@video/contracts";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), action: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../platform/transport", () => ({
  native: true,
  transport: { action: mocks.action },
}));
import Library from "./Library";
const movies: Movie[] = [
  {
    id: "1",
    title: "Đường về",
    output: "C:\\Videos\\one.mp4",
    format: "mp4",
    duration: 60,
    addedAt: 1,
    watched: false,
    missing: false,
  },
  {
    id: "2",
    title: "Phim đã xem",
    output: "C:\\Videos\\two.mkv",
    format: "mkv",
    duration: 120,
    addedAt: 2,
    watched: true,
    missing: false,
  },
];
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  vi.resetAllMocks();
  mocks.invoke.mockImplementation(async (command: string) =>
    command === "list_movies" ? movies : "data:image/jpeg;base64,/9j/",
  );
  mocks.action.mockResolvedValue(undefined);
});
afterEach(cleanup);
function mount() {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <Library />
    </QueryClientProvider>,
  );
}
it("searches without accents and filters watched/format", async () => {
  mount();
  await screen.findByText("Đường về");
  fireEvent.change(screen.getByLabelText("Tìm phim"), {
    target: { value: "duong" },
  });
  expect(screen.queryByText("Phim đã xem")).toBeNull();
  expect(screen.getByText("Đường về")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Tìm phim"), {
    target: { value: "" },
  });
  fireEvent.change(screen.getByLabelText("Lọc trạng thái xem"), {
    target: { value: "watched" },
  });
  expect(screen.queryByText("Đường về")).toBeNull();
  fireEvent.change(screen.getByLabelText("Lọc định dạng"), {
    target: { value: "mp4" },
  });
  expect(screen.getByText("Không có phim phù hợp")).toBeTruthy();
});
it("marks watched and renames with the selected movie id", async () => {
  mount();
  await screen.findByText("Đường về");
  fireEvent.click(screen.getByText("Đánh dấu đã xem"));
  await waitFor(() =>
    expect(mocks.action).toHaveBeenCalledWith("mark_movie_watched", {
      id: "1",
      watched: true,
    }),
  );
  fireEvent.click(screen.getByLabelText("Đổi tên Đường về"));
  fireEvent.change(screen.getByLabelText("Tên phim mới"), {
    target: { value: "Tên mới" },
  });
  fireEvent.click(screen.getByText("Lưu tên mới"));
  await waitFor(() =>
    expect(mocks.action).toHaveBeenCalledWith("rename_movie", {
      id: "1",
      name: "Tên mới",
    }),
  );
});
it("requires explicit confirmation before deleting a video", async () => {
  mount();
  await screen.findByText("Đường về");
  fireEvent.click(screen.getByLabelText("Xóa Đường về"));
  expect(mocks.action).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Giữ lại"));
  expect(mocks.action).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText("Xóa Đường về"));
  fireEvent.click(screen.getByText("Xóa video"));
  await waitFor(() =>
    expect(mocks.action).toHaveBeenCalledWith("delete_movie", { id: "1" }),
  );
});

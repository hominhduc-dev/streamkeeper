import { useState, useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  Film,
  FolderOpen,
  Play,
  Check,
  Trash2,
  Pencil,
  Search,
} from "lucide-react";
import { Button, Input } from "@video/ui";
import type { Movie } from "@video/contracts";
import { native, transport } from "../platform/transport";
import { duration } from "./format";

function Poster({ movie }: { movie: Movie }) {
  const poster = useQuery({
    queryKey: ["poster", movie.id],
    queryFn: () => invoke<string>("movie_poster", { id: movie.id }),
    enabled: native && !movie.missing,
    retry: false,
    staleTime: Infinity,
  });
  return (
    <div className="movie-poster">
      {poster.data ? (
        <img src={poster.data} alt={`Poster ${movie.title}`} loading="lazy" />
      ) : (
        <Film size={48} />
      )}
      <span>
        {movie.format.toUpperCase()} · {duration(movie.duration)}
      </span>
    </div>
  );
}
export default function Library() {
  const dialog = useRef<HTMLDialogElement>(null);
  const query = useQueryClient();
  const movies = useQuery({
    queryKey: ["movies"],
    queryFn: () => invoke<Movie[]>("list_movies"),
    enabled: native,
    refetchInterval: 10000,
  });
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState("all");
  const [format, setFormat] = useState("all");
  const [selected, setSelected] = useState<{
    movie: Movie;
    mode: "rename" | "delete";
  } | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (selected && !dialog.current?.open) dialog.current?.showModal();
  }, [selected]);
  const normalize = (s: string) =>
    s
      .toLocaleLowerCase("vi")
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/đ/g, "d")
      .toLocaleLowerCase("vi");
  const items = (movies.data ?? []).filter(
    (m) =>
      normalize(m.title).includes(normalize(search)) &&
      (format === "all" || m.format === format) &&
      (filter === "all" ||
        (filter === "watched" && m.watched) ||
        (filter === "unwatched" && !m.watched) ||
        (filter === "missing" && m.missing)),
  );
  async function action(command: string, args: Record<string, unknown>) {
    setBusy(true);
    setError("");
    try {
      await transport.action(command, args);
      await Promise.all([
        query.invalidateQueries({ queryKey: ["movies"] }),
        query.invalidateQueries({ queryKey: ["jobs"] }),
      ]);
      setSelected(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">PHIM CỦA BẠN</div>
          <h1>Thư viện phim.</h1>
          <p>
            {movies.data?.length ?? 0} phim đã tải · Poster được tạo từ khung
            hình video.
          </p>
        </div>
      </div>
      <div className="library-filters">
        <label className="search-box">
          <Search size={18} />
          <Input
            aria-label="Tìm phim"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Tìm tên phim…"
          />
        </label>
        <select
          aria-label="Lọc trạng thái xem"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">Tất cả trạng thái</option>
          <option value="unwatched">Chưa xem</option>
          <option value="watched">Đã xem</option>
          <option value="missing">Không tìm thấy file</option>
        </select>
        <select
          aria-label="Lọc định dạng"
          value={format}
          onChange={(e) => setFormat(e.target.value)}
        >
          <option value="all">Mọi định dạng</option>
          <option value="mp4">MP4</option>
          <option value="mkv">MKV</option>
        </select>
      </div>
      {!selected && (error || movies.error) && (
        <div role="alert" className="notice">
          {error || String(movies.error)}
        </div>
      )}
      {movies.isLoading ? (
        <p>Đang đọc thư viện…</p>
      ) : items.length ? (
        <div className="movie-grid">
          {items.map((movie) => (
            <article className="movie-card" key={movie.id}>
              <Poster movie={movie} />
              <div className="movie-body">
                <h2 title={movie.title}>{movie.title}</h2>
                <span
                  className={`badge ${movie.missing ? "badge-failed" : movie.watched ? "badge-completed" : "badge-paused"}`}
                >
                  {movie.missing
                    ? "Không tìm thấy file"
                    : movie.watched
                      ? "Đã xem"
                      : "Chưa xem"}
                </span>
                <p className="movie-path" title={movie.output}>
                  {movie.output}
                </p>
                <div className="movie-actions">
                  <Button
                    disabled={busy || movie.missing}
                    onClick={() =>
                      action("open_artifact", { id: movie.id, folder: false })
                    }
                  >
                    <Play size={15} />
                    Xem phim
                  </Button>
                  <Button
                    variant="ghost"
                    aria-label={`Mở thư mục ${movie.title}`}
                    disabled={busy}
                    onClick={() =>
                      action("open_artifact", { id: movie.id, folder: true })
                    }
                  >
                    <FolderOpen size={16} />
                  </Button>
                </div>
                <div className="movie-actions">
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() =>
                      action("mark_movie_watched", {
                        id: movie.id,
                        watched: !movie.watched,
                      })
                    }
                  >
                    <Check size={15} />
                    {movie.watched ? "Đánh dấu chưa xem" : "Đánh dấu đã xem"}
                  </Button>
                  <Button
                    variant="ghost"
                    aria-label={`Đổi tên ${movie.title}`}
                    disabled={busy || movie.missing}
                    onClick={() => {
                      setName(movie.title);
                      setError("");
                      setSelected({ movie, mode: "rename" });
                    }}
                  >
                    <Pencil size={15} />
                  </Button>
                  <Button
                    variant="ghost"
                    aria-label={`Xóa ${movie.title}`}
                    disabled={busy}
                    onClick={() => {
                      setError("");
                      setSelected({ movie, mode: "delete" });
                    }}
                  >
                    <Trash2 size={15} />
                  </Button>
                </div>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="empty-state">
          <Film size={42} />
          <h2>
            {search || filter !== "all" || format !== "all"
              ? "Không có phim phù hợp"
              : "Thư viện đang trống"}
          </h2>
          <p>Video tải xong sẽ tự xuất hiện tại đây.</p>
        </div>
      )}
      {selected && (
        <dialog
          ref={dialog}
          onCancel={(e) => {
            e.preventDefault();
            if (!busy) setSelected(null);
          }}
          className="modal"
          role="dialog"
          aria-modal="true"
          aria-label={selected.mode === "rename" ? "Đổi tên phim" : "Xóa phim"}
        >
          <h2>
            {selected.mode === "rename"
              ? "Đổi tên phim"
              : "Xóa phim khỏi ổ đĩa?"}
          </h2>
          <p>
            {selected.mode === "rename"
              ? "Tên mới áp dụng cho tên phim và file video. Phụ đề rời giữ nguyên tên."
              : `Xóa video “${selected.movie.title}” và bỏ khỏi thư viện. Phụ đề rời và lịch sử được giữ lại. Thao tác này không thể hoàn tác.`}
          </p>
          {selected.mode === "rename" && (
            <Input
              autoFocus
              aria-label="Tên phim mới"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          )}
          {error && <p role="alert">{error}</p>}
          <div className="movie-actions">
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => setSelected(null)}
            >
              Giữ lại
            </Button>
            <Button
              disabled={busy || (selected.mode === "rename" && !name.trim())}
              onClick={() =>
                action(
                  selected.mode === "rename" ? "rename_movie" : "delete_movie",
                  selected.mode === "rename"
                    ? { id: selected.movie.id, name: name.trim() }
                    : { id: selected.movie.id },
                )
              }
            >
              {busy
                ? "Đang xử lý…"
                : selected.mode === "rename"
                  ? "Lưu tên mới"
                  : "Xóa video"}
            </Button>
          </div>
        </dialog>
      )}
    </>
  );
}

import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { NavLink, Route, Routes, useLocation } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowDownToLine,
  ArrowUpRight,
  Check,
  CheckCircle2,
  ChevronRight,
  Clock3,
  Download,
  Film,
  FolderOpen,
  HardDrive,
  History,
  Layers3,
  LoaderCircle,
  Pause,
  Play,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Subtitles,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import { Button, Card, Input } from "@video/ui";
import type {
  Inspection,
  Job,
  Settings,
  DownloadOptions,
} from "@video/contracts";
import { transport, native } from "../platform/transport";
import { bytes, duration, percentage, states } from "./format";

function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal"
      aria-label={title}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <header>
        <h2>{title}</h2>
        <Button
          variant="ghost"
          type="button"
          onClick={onClose}
          aria-label="Đóng"
        >
          <X size={18} />
        </Button>
      </header>
      {children}
    </dialog>
  );
}
function Empty({ history, onAdd }: { history?: boolean; onAdd: () => void }) {
  return (
    <div className="empty">
      <div className="empty-art">
        <Film size={35} />
        <span>
          <ArrowDownToLine size={17} />
        </span>
      </div>
      <h3>
        {history
          ? "Chưa có video trong lịch sử"
          : "Video tiếp theo bắt đầu từ một đường dẫn"}
      </h3>
      <p>
        {history
          ? "Các tác vụ hoàn tất hoặc đã hủy sẽ xuất hiện tại đây."
          : "Dán URL HLS hoặc trang phim được hỗ trợ. Chọn chất lượng, tiếng và phụ đề trước khi tải."}
      </p>
      <Button onClick={onAdd}>
        <Plus size={17} />
        Thêm video đầu tiên
      </Button>
      <div className="empty-tags">
        <span>HLS VOD</span>
        <span>MP4 / MKV</span>
        <span>Phụ đề tiếng Việt</span>
      </div>
    </div>
  );
}
export default function App() {
  const query = useQueryClient();
  const location = useLocation();
  const jobsQ = useQuery({
    queryKey: ["jobs"],
    queryFn: transport.jobs,
    enabled: native,
    refetchInterval: 5000,
  });
  const settingsQ = useQuery({
    queryKey: ["settings"],
    queryFn: transport.settings,
    enabled: native,
  });
  const runtimeQ = useQuery({
    queryKey: ["runtime"],
    queryFn: transport.runtime,
    enabled: native,
  });
  const jobs = jobsQ.data ?? [];
  const active = jobs.filter(
    (j) => !["completed", "cancelled"].includes(j.state),
  );
  const history = jobs.filter((j) =>
    ["completed", "cancelled", "failed"].includes(j.state),
  );
  const [add, setAdd] = useState(false);
  const [detail, setDetail] = useState<string | null>(null);
  const [cancel, setCancel] = useState<Job | null>(null);
  const [closing, setClosing] = useState(false);
  const [toast, setToast] = useState<{
    message: string;
    error?: boolean;
  } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const notify = (message: string, error = false) =>
    setToast({ message, error });
  async function action(command: string, args?: Record<string, unknown>) {
    setBusy(`${command}:${args?.id ?? ""}`);
    try {
      await transport.action(command, args);
      await query.invalidateQueries({ queryKey: ["jobs"] });
    } catch (e) {
      notify(String(e), true);
    } finally {
      setBusy(null);
    }
  }
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const addListener = (p: Promise<() => void>) =>
      p.then((fn) => {
        if (disposed) fn();
        else unlisten.push(fn);
      });
    addListener(
      transport.subscribe((job) =>
        query.setQueryData<Job[]>(["jobs"], (old) => {
          const previous = old?.find((j) => j.id === job.id);
          if (previous && previous.revision > job.revision) return old;
          return [job, ...(old ?? []).filter((j) => j.id !== job.id)].sort(
            (a, b) => b.createdAt - a.createdAt,
          );
        }),
      ),
    );
    addListener(
      listen("downloads-reconcile", () =>
        query.invalidateQueries({ queryKey: ["jobs"] }),
      ),
    );
    addListener(
      listen("close-requested", async () => {
        const current = await transport.jobs();
        if (
          current.some((j) =>
            ["queued", "downloading", "muxing", "verifying"].includes(j.state),
          )
        )
          setClosing(true);
        else await transport.action("shutdown_app");
      }),
    );
    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, [query]);
  useEffect(() => {
    const theme = settingsQ.data?.theme ?? "dark";
    const apply = () =>
      (document.documentElement.dataset.theme =
        theme === "system"
          ? matchMedia("(prefers-color-scheme:dark)").matches
            ? "dark"
            : "light"
          : theme);
    apply();
    const m = matchMedia("(prefers-color-scheme:dark)");
    m.addEventListener("change", apply);
    return () => m.removeEventListener("change", apply);
  }, [settingsQ.data?.theme]);
  const selected = jobs.find((j) => j.id === detail);
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-icon">
            <ArrowDownToLine size={22} />
          </div>
          <div>
            streamkeeper<span>YOUR VIDEOS, KEPT LOCAL</span>
          </div>
        </div>
        <div className="nav-caption">THƯ VIỆN CỦA BẠN</div>
        <nav>
          <NavLink to="/" end>
            <Download size={18} />
            Đang tải<span className="nav-count">{active.length}</span>
          </NavLink>
          <NavLink to="/history">
            <History size={18} />
            Lịch sử<span className="nav-count">{history.length}</span>
          </NavLink>
          <NavLink to="/settings">
            <Settings2 size={18} />
            Cài đặt
          </NavLink>
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <ShieldCheck size={20} />
            <strong>Chạy trên máy của bạn</strong>
            <p>Video và phiên tải được xử lý local.</p>
          </div>
          <span className="version">
            Streamkeeper <span>v0.1.0</span>
          </span>
        </div>
      </aside>
      <main>
        <div className="topbar">
          <span>
            Không gian làm việc <ChevronRight size={13} />
            <b>
              {location.pathname === "/history"
                ? "Lịch sử"
                : location.pathname === "/settings"
                  ? "Cài đặt"
                  : "Downloads"}
            </b>
          </span>
          <div>
            <i
              className={
                native && runtimeQ.data?.ffmpeg && runtimeQ.data.ffprobe
                  ? "status-dot"
                  : "status-dot warning"
              }
            />
            {native ? "Desktop local" : "Xem giao diện trong browser"}
          </div>
        </div>
        {!native && (
          <div className="notice">
            <TriangleAlert size={17} />
            Hãy mở ứng dụng Windows để phân tích nguồn và tải video. Giao diện
            browser không có engine desktop.
          </div>
        )}
        {jobsQ.isError && (
          <div className="notice error">
            Không đọc được tác vụ: {String(jobsQ.error)}
            <Button variant="ghost" onClick={() => jobsQ.refetch()}>
              Thử lại
            </Button>
          </div>
        )}
        <Routes>
          <Route
            path="/"
            element={
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">TẢI VỀ. GIỮ LẠI.</div>
                    <h1>Video của bạn.</h1>
                    <p>
                      Chọn chất lượng bạn muốn. Phần còn lại để Streamkeeper.
                    </p>
                  </div>
                  <Button disabled={!native} onClick={() => setAdd(true)}>
                    <Plus size={18} />
                    Thêm video
                  </Button>
                </div>
                <div className="stats">
                  <Card>
                    <span>
                      <Layers3 size={17} />
                      ĐANG XỬ LÝ
                    </span>
                    <strong>
                      {
                        active.filter((j) =>
                          ["downloading", "muxing", "verifying"].includes(
                            j.state,
                          ),
                        ).length
                      }
                      <small> tác vụ</small>
                    </strong>
                    <p>
                      {active.filter((j) => j.state === "queued").length} đang
                      chờ trong hàng đợi
                    </p>
                  </Card>
                  <Card>
                    <span>
                      <CheckCircle2 size={17} />
                      ĐÃ HOÀN TẤT
                    </span>
                    <strong>
                      {history.filter((j) => j.state === "completed").length}
                      <small> video</small>
                    </strong>
                    <p>Sẵn sàng xem từ thư mục của bạn</p>
                  </Card>
                  <Card>
                    <span>
                      <HardDrive size={17} />
                      DỮ LIỆU ĐÃ TẢI
                    </span>
                    <strong>
                      {bytes(jobs.reduce((n, j) => n + j.downloadedBytes, 0))}
                    </strong>
                    <p>Giữ nguyên chất lượng nguồn</p>
                  </Card>
                </div>
                <section className="downloads-section">
                  <div className="section-title">
                    <h2>
                      Hàng đợi tải <span>{active.length}</span>
                    </h2>
                    <span>
                      <i className="status-dot" />
                      Một tác vụ active · {settingsQ.data?.concurrency ??
                        8}{" "}
                      luồng
                    </span>
                  </div>
                  {jobsQ.isLoading ? (
                    <div className="loading">
                      <LoaderCircle className="spin" />
                      Đang đọc tác vụ…
                    </div>
                  ) : active.length ? (
                    active.map((j) => (
                      <JobCard
                        key={j.id}
                        job={j}
                        busy={!!busy}
                        onDetail={() => setDetail(j.id)}
                        onAction={action}
                        onCancel={() => setCancel(j)}
                      />
                    ))
                  ) : (
                    <Empty
                      onAdd={() => {
                        if (native) setAdd(true);
                        else
                          notify(
                            "Chức năng tải chạy trong ứng dụng Windows.",
                            true,
                          );
                      }}
                    />
                  )}
                </section>
                <div className="bottom-tip">
                  <Subtitles size={18} />
                  <div>
                    <strong>Giữ cả những câu thoại bạn thích.</strong>
                    <span>
                      Chọn phụ đề khi thêm video để lưu riêng hoặc ghép vào
                      file.
                    </span>
                  </div>
                  <span>VTT · SRT · Soft sub</span>
                </div>
              </>
            }
          />
          <Route
            path="/history"
            element={
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">ĐÃ LƯU TRÊN MÁY</div>
                    <h1>Lịch sử tải.</h1>
                    <p>Mở lại video hoàn tất và kiểm tra các tác vụ đã hủy.</p>
                  </div>
                  <Button disabled={!native} onClick={() => setAdd(true)}>
                    <Plus size={18} />
                    Thêm video
                  </Button>
                </div>
                {history.length ? (
                  history.map((j) => (
                    <JobCard
                      key={j.id}
                      job={j}
                      busy={!!busy}
                      onDetail={() => setDetail(j.id)}
                      onAction={action}
                      onCancel={() => setCancel(j)}
                    />
                  ))
                ) : (
                  <Empty history onAdd={() => native && setAdd(true)} />
                )}
              </>
            }
          />
          <Route
            path="/settings"
            element={
              <SettingsPage
                settings={settingsQ.data}
                runtime={runtimeQ.data}
                onSave={async (s) => {
                  await transport.action("update_settings", { settings: s });
                  await query.invalidateQueries({ queryKey: ["settings"] });
                  notify("Đã lưu cài đặt.");
                }}
                onError={(e) => notify(String(e), true)}
                onDiagnostics={async () => {
                  setBusy("diagnostics");
                  try {
                    const path = await invoke<string>("export_diagnostics");
                    notify(`Đã lưu chẩn đoán: ${path}`);
                  } catch (e) {
                    notify(String(e), true);
                  } finally {
                    setBusy(null);
                  }
                }}
              />
            }
          />
        </Routes>
        <footer>
          <span>
            <ShieldCheck size={13} />
            Phiên được giữ trong engine, không đưa lên giao diện.
          </span>
          <span>HLS → Media → MP4</span>
        </footer>
      </main>
      {add && settingsQ.data && (
        <Modal title="Thêm video" onClose={() => setAdd(false)}>
          <Inspector
            settings={settingsQ.data}
            onCreated={() => {
              setAdd(false);
              query.invalidateQueries({ queryKey: ["jobs"] });
              notify("Đã thêm video vào hàng đợi.");
            }}
          />
        </Modal>
      )}
      {selected && (
        <Modal title="Chi tiết tác vụ" onClose={() => setDetail(null)}>
          <div className="detail">
            <h3>{selected.title}</h3>
            <dl>
              {[
                ["Trạng thái", states[selected.state]],
                ["Giai đoạn", selected.stage],
                ["Thời lượng", duration(selected.duration)],
                ["Đã tải", bytes(selected.downloadedBytes)],
                [
                  "Segment",
                  `${selected.completedSegments} / ${selected.totalSegments}`,
                ],
                ["Định dạng", selected.options.format.toUpperCase()],
                ["Output", selected.output ?? "—"],
                ["Lỗi", selected.error ?? "Không có"],
              ].map(([key, value]) => (
                <div key={key}>
                  <dt>{key}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
            <div className="form-actions">
              <Button
                variant="secondary"
                onClick={() =>
                  action("open_artifact", { id: selected.id, folder: true })
                }
              >
                <FolderOpen size={16} />
                Mở thư mục
              </Button>
              <Button
                variant="ghost"
                onClick={() => action("cleanup_job_cache", { id: selected.id })}
              >
                Dọn cache tác vụ
              </Button>
            </div>
          </div>
        </Modal>
      )}
      {cancel && (
        <Modal title="Hủy tác vụ?" onClose={() => setCancel(null)}>
          <p className="modal-copy">
            Dừng tải “{cancel.title}”. Video hoàn tất có sẵn sẽ không bị xóa.
          </p>
          <div className="form-actions">
            <Button
              variant="secondary"
              disabled={!!busy}
              onClick={async () => {
                await action("cancel_download", {
                  id: cancel.id,
                  keepCache: true,
                });
                setCancel(null);
              }}
            >
              Hủy, giữ cache
            </Button>
            <Button
              variant="destructive"
              disabled={!!busy}
              onClick={async () => {
                await action("cancel_download", {
                  id: cancel.id,
                  keepCache: false,
                });
                setCancel(null);
              }}
            >
              Hủy và dọn cache
            </Button>
          </div>
        </Modal>
      )}
      {closing && (
        <Modal title="Đang có tác vụ chạy" onClose={() => setClosing(false)}>
          <p className="modal-copy">
            Bạn có thể chờ hoàn tất hoặc đóng app và lưu tiến trình. Nếu đang
            ghép, lần mở lại sẽ ghép từ cache.
          </p>
          <div className="form-actions">
            <Button variant="secondary" onClick={() => setClosing(false)}>
              Chờ hoàn tất
            </Button>
            <Button disabled={!!busy} onClick={() => action("shutdown_app")}>
              Lưu tiến trình và đóng
            </Button>
          </div>
        </Modal>
      )}
      {toast && (
        <div
          role="alert"
          className={`toast ${toast.error ? "toast-error" : ""}`}
        >
          <span>{toast.message}</span>
          <Button
            variant="ghost"
            onClick={() => setToast(null)}
            aria-label="Đóng thông báo"
          >
            <X size={16} />
          </Button>
        </div>
      )}
    </div>
  );
}
function JobCard({
  job: j,
  busy,
  onDetail,
  onAction,
  onCancel,
}: {
  job: Job;
  busy: boolean;
  onDetail: () => void;
  onAction: (command: string, args?: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
}) {
  const p = percentage(j.completedSegments, j.totalSegments, j.state);
  const running = ["downloading", "muxing", "verifying"].includes(j.state);
  return (
    <Card className="job-card">
      <div className={`job-icon ${j.state === "completed" ? "finished" : ""}`}>
        {j.state === "completed" ? <Check size={24} /> : <Film size={24} />}
      </div>
      <div className="job-content">
        <div className="job-top">
          <button className="job-title" onClick={onDetail}>
            {j.title}
          </button>
          <span className={`badge badge-${j.state}`}>
            {running && <LoaderCircle size={12} className="spin" />}
            {states[j.state] ?? j.state}
          </span>
        </div>
        <div className="job-meta">
          <span>{j.options.format.toUpperCase()}</span>
          <span>{duration(j.duration)}</span>
          <span>{j.provider}</span>
          {j.options.subtitleId && (
            <span>
              <Subtitles size={13} />
              Có phụ đề
            </span>
          )}
        </div>
        <div className="progress">
          <div style={{ width: `${p}%` }} />
        </div>
        <div className="job-progress">
          <span>
            {bytes(j.downloadedBytes)}
            {j.estimatedTotalBytes
              ? ` / ~${bytes(j.estimatedTotalBytes)}`
              : ""}{" "}
            · {j.completedSegments}/{j.totalSegments} segment
          </span>
          <span>
            {j.speed > 0 ? `${bytes(j.speed)}/s · ` : ""}
            {j.eta != null ? `Còn ~${duration(j.eta)} · ` : ""}
            {p}%
          </span>
        </div>
        {j.error && (
          <p className="job-error">
            <TriangleAlert size={13} />
            {j.error}
          </p>
        )}
      </div>
      <div className="job-actions">
        {["queued", "downloading"].includes(j.state) && (
          <Button
            variant="ghost"
            disabled={busy}
            aria-label="Tạm dừng"
            onClick={() => onAction("pause_download", { id: j.id })}
          >
            <Pause size={17} />
          </Button>
        )}
        {["paused", "failed"].includes(j.state) && (
          <Button
            variant="secondary"
            disabled={busy}
            aria-label="Tiếp tục"
            onClick={() => onAction("resume_download", { id: j.id })}
          >
            <Play size={16} />
          </Button>
        )}
        {j.state === "completed" ? (
          <Button
            variant="secondary"
            disabled={busy}
            aria-label="Mở video"
            onClick={() =>
              onAction("open_artifact", { id: j.id, folder: false })
            }
          >
            <ArrowUpRight size={17} />
          </Button>
        ) : (
          j.state !== "cancelled" && (
            <Button
              variant="ghost"
              disabled={busy}
              aria-label="Hủy tác vụ"
              onClick={onCancel}
            >
              <X size={16} />
            </Button>
          )
        )}
        <Button
          variant="ghost"
          disabled={busy}
          aria-label="Mở thư mục"
          onClick={() => onAction("open_artifact", { id: j.id, folder: true })}
        >
          <FolderOpen size={17} />
        </Button>
      </div>
    </Card>
  );
}
function Inspector({
  settings,
  onCreated,
}: {
  settings: Settings;
  onCreated: () => void;
}) {
  const [source, setSource] = useState("");
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [video, setVideo] = useState("");
  const [audio, setAudio] = useState("");
  const [subtitle, setSubtitle] = useState("");
  const [mode, setMode] = useState("none");
  const [offset, setOffset] = useState(0);
  const [format, setFormat] = useState("mp4");
  const [filename, setFilename] = useState("");
  const [dir, setDir] = useState(settings.outputDir);
  const selected = inspection?.videoTracks.find((t) => t.id === video);
  const audios =
    inspection?.audioTracks.filter(
      (t) => t.audioGroup === selected?.audioGroup,
    ) ?? [];
  async function analyze(e: FormEvent) {
    e.preventDefault();
    setError("");
    setBusy(true);
    setInspection(null);
    try {
      const url = new URL(source);
      if (!["http:", "https:"].includes(url.protocol))
        throw new Error("Hãy nhập URL HTTP hoặc HTTPS.");
      const result = await transport.inspect(source.trim());
      setInspection(result);
      const chosen =
        result.videoTracks.find((t) => t.resolution?.startsWith("1920")) ??
        result.videoTracks[0];
      setVideo(chosen.id);
      setAudio(
        result.audioTracks.find((t) => t.audioGroup === chosen.audioGroup)
          ?.id ?? "",
      );
      setFilename(
        result.title
          .replace(/[<>:"/\\|?*]/g, "-")
          .trim()
          .slice(0, 160) || "Video",
      );
      setSubtitle("");
      setMode("none");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function create(e: FormEvent) {
    e.preventDefault();
    if (!inspection) return;
    setBusy(true);
    setError("");
    try {
      const options: DownloadOptions = {
        inspectionId: inspection.id,
        videoId: video,
        audioId: audio || null,
        subtitleId: subtitle || null,
        subtitleMode: subtitle ? mode : "none",
        subtitleOffset: offset,
        outputDir: dir,
        filename,
        format,
      };
      await transport.create(options);
      onCreated();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="inspector">
      <form onSubmit={analyze}>
        <label htmlFor="source">Đường dẫn video</label>
        <div className="url-row">
          <Input
            id="source"
            value={source}
            onChange={(e) => {
              setSource(e.target.value);
              setInspection(null);
            }}
            placeholder="https://…/master.m3u8 hoặc trang Film4k"
            required
            disabled={busy}
          />
          <Button disabled={busy}>
            {busy ? (
              <LoaderCircle size={17} className="spin" />
            ) : (
              <Search size={17} />
            )}
            Phân tích
          </Button>
        </div>
        <p className="hint">
          HLS VOD hoặc nguồn Film4k. Nguồn có DRM chưa được hỗ trợ.
        </p>
      </form>
      {error && (
        <div className="form-error" role="alert">
          {error}
        </div>
      )}
      {inspection && (
        <form onSubmit={create} className="track-form">
          <div className="source-result">
            <Film size={24} />
            <div>
              <strong>{inspection.title}</strong>
              <span>
                {inspection.provider} · {duration(inspection.duration)} ·{" "}
                {inspection.videoTracks.length} chất lượng
              </span>
            </div>
            <CheckCircle2 size={19} />
          </div>
          <div className="form-grid">
            <label>
              Chất lượng
              <select
                value={video}
                onChange={(e) => {
                  setVideo(e.target.value);
                  const t = inspection.videoTracks.find(
                    (t) => t.id === e.target.value,
                  );
                  setAudio(
                    inspection.audioTracks.find(
                      (a) => a.audioGroup === t?.audioGroup,
                    )?.id ?? "",
                  );
                }}
              >
                {inspection.videoTracks.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label}
                    {t.bandwidth
                      ? ` · ${(t.bandwidth / 1e6).toFixed(1)} Mbps`
                      : ""}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Audio
              <select
                value={audio}
                onChange={(e) => setAudio(e.target.value)}
                disabled={!audios.length}
              >
                {audios.length ? (
                  audios.map((t) => (
                    <option value={t.id} key={t.id}>
                      {t.label} {t.language ? `(${t.language})` : ""}
                    </option>
                  ))
                ) : (
                  <option value="">Tiếng có sẵn trong video</option>
                )}
              </select>
            </label>
            <label>
              Phụ đề
              <select
                value={subtitle}
                onChange={(e) => {
                  setSubtitle(e.target.value);
                  setMode(e.target.value ? "soft" : "none");
                }}
              >
                <option value="">Không tải phụ đề</option>
                {inspection.subtitleTracks.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label} ({t.language ?? "und"})
                  </option>
                ))}
              </select>
            </label>
            <label>
              Định dạng
              <select
                value={format}
                onChange={(e) => setFormat(e.target.value)}
              >
                <option value="mp4">MP4</option>
                <option value="mkv">MKV</option>
              </select>
            </label>
            {subtitle && (
              <>
                <label>
                  Cách lưu phụ đề
                  <select
                    value={mode}
                    onChange={(e) => setMode(e.target.value)}
                  >
                    <option value="soft">Ghép vào video, bật/tắt được</option>
                    <option value="srt">Lưu riêng SRT</option>
                    <option value="vtt">Lưu riêng VTT</option>
                  </select>
                </label>
                <label>
                  Độ lệch phụ đề (giây)
                  <Input
                    type="number"
                    step="0.1"
                    min="-86400"
                    max="86400"
                    value={offset}
                    onChange={(e) => setOffset(Number(e.target.value))}
                  />
                </label>
              </>
            )}
          </div>
          <label>
            Tên file
            <Input
              required
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
            />
            <span className="hint">
              Phần mở rộng .{format} được thêm tự động.
            </span>
          </label>
          <label>
            Nơi lưu
            <div className="url-row">
              <Input
                required
                value={dir}
                onChange={(e) => setDir(e.target.value)}
              />
              <Button
                type="button"
                variant="secondary"
                aria-label="Chọn thư mục"
                onClick={async () => {
                  try {
                    const value = await open({
                      directory: true,
                      multiple: false,
                      defaultPath: dir,
                    });
                    if (typeof value === "string") setDir(value);
                  } catch (e) {
                    setError(String(e));
                  }
                }}
              >
                <FolderOpen size={18} />
              </Button>
            </div>
          </label>
          <div className="form-actions">
            <span className="hint">
              Giữ nguyên chất lượng · Không encode lại
            </span>
            <Button disabled={busy}>
              {busy ? (
                <LoaderCircle size={16} className="spin" />
              ) : (
                <ArrowDownToLine size={16} />
              )}
              Thêm vào hàng đợi
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
function SettingsPage({
  settings,
  runtime,
  onSave,
  onError,
  onDiagnostics,
}: {
  settings?: Settings;
  runtime?: import("@video/contracts").RuntimeStatus;
  onSave: (s: Settings) => Promise<void>;
  onError: (e: unknown) => void;
  onDiagnostics: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (settings) setDraft(settings);
  }, [settings]);
  async function save(e: FormEvent) {
    e.preventDefault();
    if (!draft) return;
    setSaving(true);
    try {
      await onSave(draft);
    } catch (e) {
      onError(e);
    } finally {
      setSaving(false);
    }
  }
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">THEO CÁCH CỦA BẠN</div>
          <h1>Cài đặt.</h1>
          <p>Nơi lưu video, tốc độ tải và cách giữ dữ liệu tạm.</p>
        </div>
        <SlidersHorizontal size={26} />
      </div>
      {draft ? (
        <form onSubmit={save}>
          <Card className="settings-card">
            <h2>Lưu trữ & tải về</h2>
            <label>
              Thư mục video
              <Input
                value={draft.outputDir}
                onChange={(e) =>
                  setDraft({ ...draft, outputDir: e.target.value })
                }
                required
              />
            </label>
            <label>
              Thư mục cache
              <Input
                value={draft.cacheDir}
                onChange={(e) =>
                  setDraft({ ...draft, cacheDir: e.target.value })
                }
                required
              />
              <span className="hint">
                Không đổi thư mục cache khi còn tác vụ cần tiếp tục.
              </span>
            </label>
            <div className="form-grid">
              <label>
                Số request song song
                <Input
                  type="number"
                  min="1"
                  max="16"
                  value={draft.concurrency}
                  onChange={(e) =>
                    setDraft({ ...draft, concurrency: Number(e.target.value) })
                  }
                />
              </label>
              <label>
                Giao diện
                <select
                  value={draft.theme}
                  onChange={(e) =>
                    setDraft({ ...draft, theme: e.target.value })
                  }
                >
                  <option value="dark">Tối</option>
                  <option value="light">Sáng</option>
                  <option value="system">Theo hệ thống</option>
                </select>
              </label>
            </div>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={draft.keepCache}
                onChange={(e) =>
                  setDraft({ ...draft, keepCache: e.target.checked })
                }
              />
              <span>
                Giữ cache sau khi hoàn tất
                <small>
                  Cache của tác vụ tạm dừng hoặc lỗi luôn được giữ để tiếp tục.
                </small>
              </span>
            </label>
            <div className="form-actions">
              <Button disabled={saving}>
                {saving && <LoaderCircle className="spin" size={16} />}Lưu cài
                đặt
              </Button>
            </div>
          </Card>
        </form>
      ) : (
        <Card className="settings-card">
          {native
            ? "Đang đọc cài đặt…"
            : "Cài đặt khả dụng trong ứng dụng Windows."}
        </Card>
      )}
      <Card className="settings-card">
        <h2>Công cụ media</h2>
        <div className="runtime-row">
          <span>FFmpeg</span>
          <span
            className={`badge ${runtime?.ffmpeg ? "badge-completed" : "badge-failed"}`}
          >
            {runtime?.ffmpeg ? "Sẵn sàng" : "Chưa xác nhận"}
          </span>
        </div>
        <div className="runtime-row">
          <span>FFprobe</span>
          <span
            className={`badge ${runtime?.ffprobe ? "badge-completed" : "badge-failed"}`}
          >
            {runtime?.ffprobe ? "Sẵn sàng" : "Chưa xác nhận"}
          </span>
        </div>
        {runtime?.ffmpegVersion && (
          <p className="hint mono">{runtime.ffmpegVersion}</p>
        )}
        <Button
          type="button"
          variant="secondary"
          disabled={!native}
          onClick={onDiagnostics}
        >
          Xuất chẩn đoán đã che thông tin phiên
        </Button>
      </Card>
    </>
  );
}

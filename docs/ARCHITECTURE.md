# Kiến trúc Streamkeeper

## Tech stack

| Phần | Công nghệ | Trách nhiệm |
|---|---|---|
| Desktop | Tauri 2 + WebView2 | Windows shell, IPC, chọn thư mục, mở output |
| UI | React 19, TypeScript, Vite, Tailwind 4 | Màn hình tải/lịch sử/cài đặt, form chọn track |
| UI state | TanStack Query, React Router | Snapshot, đồng bộ event, điều hướng |
| Engine | Rust, Tokio | Hàng đợi, tải bounded concurrency, pause/resume |
| HTTP | reqwest + rustls + cookie jar | Session, cookie scope, timeout, retry, ticket |
| Storage | SQLite WAL + SQLx | Job, settings, source URL, checkpoint |
| Media | FFmpeg + ffprobe sidecar | Remux, subtitle conversion, kiểm tra output |

Không cần server hoặc SSR. Mọi xử lý diễn ra trên máy người dùng.

Từ 0.2.0: module `crates/engine/src/library.rs` và màn hình Library quản lý job completed như phim. Migration 0002 lưu watched/deleted; poster JPEG local được tạo bởi FFmpeg và trả về dưới dạng data URI. SQLite dùng một connection để tuần tự hóa ghi checkpoint. Xem LIBRARY.md.

## Cấu trúc

```text
apps/desktop/
  src/app/              UI và kiểm thử tương tác
  src/platform/         Tauri transport
  src-tauri/            Commands, lifecycle, capabilities, sidecars
packages/
  contracts/            DTO sinh từ Rust bằng ts-rs
  sdk/                  Interface transport
  ui/                   Component dùng lại
crates/
  domain/               DTO và validation
  storage/              SQLite, migrations, checkpoints
  providers/            HLS và Film4k session/adapters
  media/                Playlist parser, PNG transform, subtitle, FFmpeg
  engine/               Job lifecycle, download và publish
scripts/                Chuẩn bị media tools
docs/                   SRS, kiến trúc, kiểm thử, licenses
.github/workflows/      Windows CI
```

## Luồng xử lý

URL → inspect → track selection → queued → downloading → muxing → verifying → completed.

Tạm dừng và shutdown hủy request/process đang chạy bằng CancellationToken. Restart đưa các job chưa kết thúc về paused. Resume tạo session mới, đọc lại manifest và đối chiếu fingerprint, chỉ tái sử dụng segment đúng SHA-256. URL ký có thể đổi nhưng nội dung track/sequence/duration phải tương thích.

Cookie jar theo domain/path; ticket single-flight chỉ gửi tới endpoint Film4k được chỉ định. Không chuyển toàn bộ cookie hay ticket sang CDN khác. Session/ticket nằm trong bộ nhớ. DTO gửi UI bỏ URL track và source. SQLite vẫn chứa URL nguồn để resume; diagnostics không xuất URL/cookie/ticket.

Segment tải streaming vào `.part`, kiểm tra byte range và payload, bỏ PNG nếu adapter cho phép, ghi checksum rồi chuyển sang cache hoàn chỉnh. FFmpeg chỉ đọc playlist cục bộ do engine tạo với protocol whitelist. Engine không chạy shell từ input người dùng. Kiểm tra output gồm duration, video/audio, packet scan và decode mẫu đầu/giữa/cuối; đây không phải decode toàn bộ từng frame.

File video tạm nằm cùng thư mục output, được kiểm tra trước rồi công bố bằng hard link không ghi đè (NTFS). File tạm được dọn khi pipeline lỗi. Sidecar subtitle được xuất trước video; nếu publish video thất bại có thể còn sidecar cần người dùng xử lý. Cleanup chỉ xóa cache UUID con trực tiếp trong cache root.

## Quyết định MVP và giới hạn

- Job/track/settings lưu JSON trong SQLite; segment checkpoint có bảng riêng. Không triển khai từng bảng chuẩn hóa như mô hình gợi ý trong SRS.
- Error code hiện dùng DOWNLOAD_FAILED chung; message có chi tiết và ẩn URL. Chưa có taxonomy lỗi chi tiết.
- HLS AES và DRM dừng rõ ràng; không có giải mã hay license workflow.
- Chưa có extension hoặc browser session bridge. Film4k được hỗ trợ bằng session HTTP riêng.
- Chưa có auto-update, signing certificate, portable installer hay web backend. Web UI chỉ là lớp giao diện.

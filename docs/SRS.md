# SRS — Ứng dụng tải video trên Windows

**Phiên bản:** 1.0 — Bản dự thảo chờ duyệt  
**Ngày:** 07/10/2026  
**Nền tảng:** Windows 10/11 x64  
**Stack đề xuất:** Tauri 2 + Rust + React + TypeScript + FFmpeg  
**Trạng thái:** Chưa triển khai dự án. Chỉ bắt đầu code sau khi chủ dự án duyệt SRS.

## 1. Mục tiêu

Xây dựng ứng dụng desktop cho phép nhập URL nguồn được hỗ trợ, chọn chất lượng, audio và phụ đề, tải segment và tạo file video hoàn chỉnh trên máy người dùng. Ứng dụng xử lý session, cookie, ticket, retry và khôi phục tác vụ sau gián đoạn; người dùng không cần thao tác terminal.

Đối tượng sử dụng là người dùng cá nhân trên Windows. Ngôn ngữ giao diện mặc định là tiếng Việt. Sản phẩm v1 hoạt động local, không cần tài khoản cloud hoặc VPS.

SRS là Software Requirements Specification. SSR/Next.js/web server không nằm trong phạm vi v1. Nếu cần webapp sau này, tái sử dụng Rust engine qua host API riêng.

## 2. Phạm vi phiên bản v1

### 2.1. Bao gồm

- App Windows có installer, đóng gói FFmpeg/FFprobe và kiểm tra runtime khi khởi động.
- HLS VOD thông thường, master/media playlist, video/audio tách và phụ đề WebVTT.
- Adapter cho nguồn Film4k đã khảo sát: khởi tạo session/ticket, header phù hợp và xử lý media có PNG prefix.
- Nhập URL, phân tích track, tải về, hàng đợi, pause/resume/cancel, lịch sử và cài đặt.
- Lưu tác vụ và manifest segment vào SQLite; khôi phục sau đóng app hoặc lỗi mạng.
- MP4 mặc định; MKV là lựa chọn fallback nếu codec/subtitle không tương thích với MP4.
- Phụ đề tải riêng SRT/VTT hoặc soft subtitle bật/tắt trong container phù hợp.
- Kiểm tra đầu ra bằng FFprobe, đọc packet toàn file và giải mã mẫu.
- Thư viện lõi Rust tách khỏi Tauri để có thể sử dụng trong CLI/backend sau này.

### 2.2. Chưa bao gồm

- Cam kết hỗ trợ mọi website, DRM, live recording, playlist mã hóa cần luồng key/license riêng.
- Đăng nhập tự động bằng cách lấy cookie từ trình duyệt đang dùng.
- Đồng bộ tài khoản cloud, webapp, remote control, tải batch cả series và mobile.
- Burn-in phụ đề hoặc encode lại video tự động. Trường hợp cần encode sẽ báo rõ thay vì chạy tác vụ tốn nhiều giờ.
- Tự động sửa phụ đề không khớp phiên bản phim; chỉ hỗ trợ điều chỉnh offset cố định.

## 3. Giả định và giới hạn

- Chủ dự án duyệt stack Tauri + Rust trước triển khai; số phiên bản cụ thể được khóa qua lockfile khi scaffold.
- Adapter chỉ xử lý nguồn có cơ chế truy cập được hỗ trợ; website thay đổi có thể cần cập nhật adapter.
- Video, cache và database nằm trên máy người dùng. Giao diện không chứa cookie/token thô.
- Không được coi video hoàn chỉnh chỉ vì file tồn tại hoặc tiến trình FFmpeg kết thúc.
- Không hứa mỗi request 403 có thể được sửa bằng refresh ticket; adapter phải nhận diện nguyên nhân phục hồi được.
- Bản v1 chọn một video rendition, một audio track và tối đa một subtitle track cho mỗi output.

## 4. Tech stack và vai trò

| Thành phần | Công nghệ | Vai trò |
|---|---|---|
| Desktop shell | Tauri 2 | Window, IPC, dialog, Windows packaging |
| Frontend | React + TypeScript + Vite | Giao diện local |
| UI | Tailwind CSS + shadcn/ui | Component, theme, layout |
| Routing | React Router | Downloads, history, settings |
| Async UI | TanStack Query + Tauri events | Snapshot tác vụ và cập nhật tiến trình |
| UI state | React state/context | Form, modal, lựa chọn track |
| Engine | Rust + Tokio | Scheduler, state machine, cancellation |
| HTTP | reqwest + cookie jar | Session, streaming response, connection reuse |
| Serialization/error | serde + thiserror | DTO và lỗi có cấu trúc |
| Storage | SQLite + SQLx | Jobs, checkpoints, settings, migrations |
| Media | FFmpeg + FFprobe native | Remux, subtitle conversion, validation |
| Logs | tracing | Chẩn đoán, che dữ liệu phiên |
| Contracts | DTO Rust + sinh TypeScript types | Tránh frontend/backend lệch kiểu |
| Workspace | pnpm workspace + Cargo workspace | Quản lý package và crates |
| Tests/CI | Rust tests, frontend tests, GitHub Actions Windows | Kiểm chứng engine, UI và build installer |

Không thêm Go, Python runtime, Redis hoặc Next.js vào sản phẩm v1. Script Python đã chạy là tài liệu tham khảo hành vi, được thay bằng Rust engine.

## 5. Kiến trúc

```text
React UI
  → Tauri commands/event bridge
    → Rust application engine
      ├── Source providers / HTTP sessions
      ├── HLS parser / media transforms
      ├── Segment scheduler / cache
      ├── SQLite repositories
      └── FFmpeg + FFprobe processes
```

- Tauri commands là lớp vận chuyển mỏng, không chứa logic downloader.
- Engine không phụ thuộc Tauri hay framework HTTP server.
- Provider chứa cơ chế truy cập và biến đổi đặc thù website; engine dùng interface chung.
- UI nhận metadata, settings và progress, không nhận media buffer nhiều GB.
- Segment được stream xuống file tạm, kiểm tra, rồi đổi tên/checkpoint. Không giữ toàn bộ video trong RAM.
- Không cần proxy mạng cục bộ cho quy trình tải hoàn tất trước rồi ghép. FFmpeg đọc playlist local của các segment đã kiểm tra. Proxy chỉ được bổ sung nếu một yêu cầu thực tế buộc cần streaming.

## 6. Cấu trúc thư mục

```text
video-downloader/
├── apps/desktop/
│   ├── src/
│   │   ├── app/                  # Bootstrap, router, providers
│   │   ├── routes/               # Downloads, history, settings
│   │   ├── features/
│   │   │   ├── source-inspector/
│   │   │   ├── downloads/
│   │   │   ├── history/
│   │   │   └── settings/
│   │   ├── components/
│   │   └── platform/             # Tauri IPC, events, dialogs
│   └── src-tauri/
│       ├── src/{commands,events,state}/
│       ├── capabilities/
│       ├── binaries/             # FFmpeg/FFprobe đúng target
│       ├── tauri.conf.json
│       └── Cargo.toml
├── crates/
│   ├── domain/                   # Source, Track, Job, Segment, Error
│   ├── engine/
│   │   └── src/{inspect,download,resume,session,scheduler}/
│   ├── providers/
│   │   └── src/{generic_hls,film4k}/
│   ├── media/
│   │   └── src/{hls,transforms,ffmpeg,validation,subtitles}/
│   └── storage/
│       ├── src/
│       └── migrations/
├── packages/
│   ├── ui/                       # Component không phụ thuộc Tauri
│   ├── contracts/                # TypeScript DTO sinh từ Rust
│   └── sdk/                      # Transport interface và desktop impl
├── tests/
│   ├── fixtures/                 # Video ngắn kiểm soát được
│   ├── integration/
│   └── e2e/
├── docs/{srs,architecture,providers,packaging}.md
├── .github/workflows/
├── Cargo.toml
├── pnpm-workspace.yaml
└── package.json
```

Runtime data theo thư mục app của hệ điều hành:

```text
app-data/
├── app.db
├── logs/
└── jobs/<job-id>/
    ├── manifest.json             # Manifest/checkpoint phục vụ kiểm tra
    ├── segments/*.part            # Segment chưa hoàn tất
    ├── segments/<stable-id>.bin    # Segment đã kiểm tra
    ├── playlists/                 # Playlist local để ghép
    └── subtitles/
```

SQLite là nguồn trạng thái tác vụ chính. Manifest file là snapshot hỗ trợ debug/recovery, không được ghi cạnh tranh như nguồn trạng thái thứ hai. Thư mục output do người dùng chọn; không lưu cache vào cùng output mặc định.

## 7. Yêu cầu chức năng

| ID | Yêu cầu | Tiêu chí nghiệm thu |
|---|---|---|
| FR-01 | Nhập URL và phân tích nguồn | Phân biệt URL hợp lệ, nguồn hỗ trợ và nguồn không hỗ trợ; trả metadata/track hoặc lỗi rõ ràng |
| FR-02 | Chọn track | Hiển thị độ phân giải, codec nếu biết, bitrate, ngôn ngữ audio/subtitle; lưu đúng lựa chọn vào job |
| FR-03 | Chọn nơi lưu và tên file | Có dialog; kiểm tra quyền ghi, tên Windows hợp lệ và file trùng; không ghi đè mặc định |
| FR-04 | Tạo tác vụ và hàng đợi | Job được persist trước khi chạy; mặc định một job active, các job khác queued |
| FR-05 | Giữ HTTP session | Init, playlist và segment dùng client/cookie jar của phiên; không trộn phiên khác tài khoản |
| FR-06 | Quản lý ticket | Refresh trước hết hạn và single-flight khi nhiều worker cần refresh; header chỉ gửi đúng phạm vi |
| FR-07 | Tải segment | Tải init/media segment theo manifest, hỗ trợ URL tương đối và byte range; không coi HTML/response lỗi là media |
| FR-08 | Retry lỗi phục hồi được | Backoff có giới hạn; xử lý timeout, network, 429/Retry-After và transient 5xx; lỗi cuối cùng có hành động retry |
| FR-09 | Progress thực tế | Hiện bytes, segment count, tốc độ và giai đoạn; tổng/ETA nullable nếu chưa biết; không báo 100% trước completed |
| FR-10 | Pause/resume | Pause chỉ khả dụng khi downloading; dừng/cancel request phù hợp và lưu checkpoint; resume dùng cache hợp lệ |
| FR-11 | Khôi phục sau restart | Job downloading bị gián đoạn chuyển paused/interrupted; kiểm tra cache, tái tạo phiên và ánh xạ segment ổn định |
| FR-12 | Cancel | Dừng network/process của đúng job; job cancelled không xuất file hoàn chỉnh; người dùng chọn giữ hoặc dọn cache |
| FR-13 | Ghép video/audio | FFmpeg nhận playlist local, giữ timestamp và codec khi compatible; không encode lại mặc định |
| FR-14 | Phụ đề | Chọn track vi/vie hoặc ngôn ngữ khác; tải riêng SRT/VTT hoặc soft subtitle; offset âm/dương giữ trong job options |
| FR-15 | Xử lý PNG prefix | Adapter nhận diện PNG, duyệt chunk đến IEND và lấy payload; file sai/truncated bị từ chối; không xóa byte cố định |
| FR-16 | Xác minh output | Probe có track đúng, đọc toàn file và giải mã mẫu đầu/giữa/cuối; duration khớp timeline trong tolerance đã định |
| FR-17 | Lịch sử | Lưu completed/failed/cancelled, lựa chọn track, đường dẫn và metadata; cho retry từ job cũ |
| FR-18 | Mở output | Mở file/folder bằng OS; nếu file đã bị di chuyển/xóa thì báo trạng thái phù hợp |
| FR-19 | Settings | Output/cache dir, concurrency, retention cache, theme; persist và validate giới hạn |
| FR-20 | Chẩn đoán | Hiển thị lỗi thân thiện kèm mã; xuất log đã redact cookie, ticket, signed URL và dữ liệu phiên |
| FR-21 | Dọn cache | Completed chỉ cleanup sau validation; job paused/failed giữ cache mặc định; xóa lịch sử không tự xóa video |
| FR-22 | Runtime/installer | Máy sạch có thể chạy sau khi installer thiết lập runtime cần thiết; FFmpeg/FFprobe được tìm bằng app resource path |

### 7.1. Quy tắc session

- Một reqwest Client/cookie jar được reuse xuyên phiên; clone client cho worker thay vì tạo client mới.
- Cookie áp dụng domain/path/secure theo chuẩn; không copy cookie Film4k sang worker domain.
- Ticket lưu expires_at và refresh mutex; không giữ mutex trong khi tải segment.
- Sau restart ưu tiên tạo phiên mới. Không lưu credential thô trong SQLite, frontend hoặc log.
- Stable segment ID dựa trên source/rendition/sequence/range/init/discontinuity; signed URL không phải định danh cache duy nhất.
- Mặc định concurrency 8 request media/job, điều chỉnh 1–16; giới hạn global và per-host để không nhân số worker theo số job. Số cuối cùng có thể điều chỉnh sau benchmark, ghi lại trong release notes.

### 7.2. Quy tắc output

- MP4: video/audio tương thích container, subtitle text chuyển mov_text.
- MKV: lựa chọn fallback khi track hỗ trợ remux vào MKV mà MP4 không phù hợp.
- Chưa hỗ trợ codec/discontinuity gây nhu cầu encode: báo lỗi có giải thích và không tự chạy encode.
- Tạo output tạm có tên riêng; chỉ chuyển sang tên hoàn chỉnh sau validation và kiểm tra lại xung đột tên.
- Pause trong muxing/verifying không khả dụng; cancel vẫn khả dụng. Nếu app bị đóng trong muxing, ghép lại từ cache.
- Thời lượng tham chiếu là timeline rendition được chọn. Tolerance mặc định = max(1 giây, 2 frame duration, một audio packet duration); ngoại lệ timeline phải được mô tả và test riêng, không bỏ qua im lặng.

## 8. Luồng sử dụng và trạng thái

### UC-01 — Tải video hoàn chỉnh

1. Người dùng dán URL và chọn Phân tích.
2. App hiển thị title, duration, chất lượng/audio/subtitle hiện có.
3. Người dùng chọn track, thư mục, tên file và bấm Tải.
4. Job đi qua queued → downloading → muxing → verifying.
5. Khi kiểm tra thành công, app hiển thị completed và nút Mở video/Mở thư mục.
6. Nếu có lỗi, app hiển thị nguyên nhân, dữ liệu đã giữ và hành động Retry/Cancel.

### UC-02 — Tiếp tục sau gián đoạn

1. Người dùng đóng app hoặc mất mạng giữa downloading.
2. App lưu checkpoint và đánh dấu interrupted khi mở lại.
3. Người dùng chọn Tiếp tục; app kiểm tra cache, lập lại session và URL.
4. Chỉ tải segment thiếu/hỏng, rồi ghép và kiểm tra.

### UC-03 — Tải và ghép phụ đề

1. Người dùng chọn subtitle track và kiểu tải riêng/ghép soft subtitle.
2. App giữ timeline gốc, áp dụng offset nếu người dùng chọn.
3. File subtitle được parse, chuyển định dạng và kiểm tra range thời gian.
4. App không khẳng định đồng bộ lời thoại tuyệt đối chỉ vì duration khớp.

Trạng thái chính:

```text
created → inspecting → queued → downloading → muxing → verifying → completed
                          ↓          ↕             ↓
                       cancelled   paused        failed
```

State machine trong code phải có bảng transition rõ ràng. Failed lưu stage để retry đúng; không tự rerun mãi. Completed và cancelled không tự quay lại downloading; retry tạo job mới hoặc thao tác chuyển trạng thái có chủ đích, được ghi nhận.

## 9. Yêu cầu giao diện

| Màn hình | Nội dung chính |
|---|---|
| Tải mới | URL, kết quả phân tích, track selection, nơi lưu, nút tạo job |
| Downloads | Active/queued jobs, progress theo giai đoạn, pause/resume/cancel và lỗi |
| Chi tiết tác vụ | Metadata, track, segment/bytes, cache/output, log lỗi đã che thông tin phiên |
| History | Kết quả, thời gian, dung lượng, mở file/folder và retry |
| Settings | Nơi lưu/cache, concurrency, retention, theme, runtime status |

- Tiếng Việt mặc định; có layout responsive theo kích thước cửa sổ desktop.
- Loading/empty/error/success states cho từng màn hình.
- Không hiển thị token, playlist URL dài hoặc thuật ngữ nội bộ trong luồng người dùng thông thường.
- Phân biệt rõ Đang tải / Đang ghép / Đang kiểm tra; ETA chỉ áp dụng stage có dữ liệu đủ.
- Đóng app khi có job active: lựa chọn Chờ hoàn tất hoặc Dừng và lưu tiến trình. Chưa có chế độ tải nền/tray ở v1.
- Điều khiển bằng bàn phím, focus rõ và label cho icon buttons.

## 10. Yêu cầu phi chức năng

| ID | Yêu cầu | Cách kiểm chứng |
|---|---|---|
| NFR-01 | UI không bị block bởi tải/FFmpeg | Tương tác các màn hình khi job chạy; không xử lý mạng/media nặng trên main UI thread |
| NFR-02 | RAM không tăng theo tổng kích thước video | Download thử ít nhất 10 GB; buffer có giới hạn và memory profiler không thấy tích lũy media toàn file |
| NFR-03 | RAM engine có budget | Mục tiêu <256 MiB ở concurrency mặc định, không tính WebView và FFmpeg; đo và báo kết quả benchmark |
| NFR-04 | Checkpoint bền vững | Kill process tại các stage fixture; restart không làm mất job hay công bố output dở |
| NFR-05 | Thông tin phiên không bị lộ | Redaction test log/error DTO; UI không nhận credentials |
| NFR-06 | FFmpeg chạy đúng phạm vi | Spawn bằng executable xác định + argument array, không shell string; cancel chỉ dừng process của job |
| NFR-07 | File/path an toàn | Validate path/filename, kiểm tra rename conflict, cleanup chỉ trong cache job của app |
| NFR-08 | Disk-full được xử lý | Kiểm tra dung lượng trước tải và lỗi ghi runtime; không đánh dấu completed khi ghi thất bại |
| NFR-09 | Progress cập nhật có giới hạn | Emit tối đa khoảng 4 lần/giây/job, coalesce events; không ghi DB mỗi packet |
| NFR-10 | Khả năng bảo trì | Provider độc lập, engine độc lập Tauri, DTO có contract check, migrations versioned |
| NFR-11 | Build có thể tái tạo | Lockfiles, Windows CI build/test, versioning và kiểm tra installer trên máy sạch |
| NFR-12 | Phân phối FFmpeg đúng build | Ghi rõ binary provenance, version, license/notice và yêu cầu phân phối của build được chọn |

Tốc độ download/ETA không phải SLA vì phụ thuộc server nguồn và đường truyền. RAM budget là mục tiêu nghiệm thu cần đo, không phải kết quả đã xác minh.

## 11. Dữ liệu và contracts

### 11.1. Database

- jobs: id, source identity/provider, options, state, stage, created/updated/completed timestamps, error code.
- tracks: metadata các track gắn với job/inspection snapshot.
- segments: stable ID, sequence/range/init/discontinuity, state, bytes, path, validation metadata.
- artifacts: output/subtitle path, format, probe metadata, validation result.
- settings: versioned key/value configuration.

Cookie/ticket không phải dữ liệu domain trả về UI. Nếu cần persistence credential trong phiên bản sau, thiết kế OS credential store riêng.

### 11.2. IPC và events

Commands: inspect_source, create_download, pause_download, resume_download, cancel_download, list_downloads, get_download, retry_download, get_settings, update_settings, open_artifact, cleanup_job_cache.

DownloadSnapshot: job_id, revision, state, stage, downloaded_bytes, estimated_total_bytes, completed_segments, total_segments, speed, eta, output, error.

DownloadEvent: job_id, revision, event_type, timestamp, payload. UI lấy snapshot lúc mở màn hình; reconnect/resubscribe đối chiếu revision và lấy lại snapshot khi có khoảng trống, không giả định event luôn được nhận.

## 12. Kiểm thử và nghiệm thu

- Fixture server local mô phỏng HTTP session, cookie domain/path, ticket expiry, 403 có/không phục hồi, 429 và network failure.
- HLS fixture có video/audio tách, relative URLs, init segment, byte range và timeline discontinuity được hỗ trợ.
- PNG-prefix fixture có payload hợp lệ, malformed chunks, truncated response và ảnh không có payload.
- Resume test khi URL token đổi, .part dở, cache hỏng và app restart.
- Subtitle fixture Unicode tiếng Việt, vi/vie aliases, offset và parse/conversion.
- E2E tất cả màn hình và named user flows trong mục 9.
- File lớn >4 GB, long-duration fixture và pipeline validation đầu-giữa-cuối.
- Network nguồn thật dùng kiểm tra adapter bổ sung; CI không phụ thuộc website bên ngoài.
- MP4 gốc và file người dùng có sẵn không được ghi đè trong quá trình test.

**Definition of Done v1:** tất cả FR-01…FR-22 có bằng chứng test hoặc manual verification; installer chạy trên Windows sạch; nguồn generic HLS và adapter khảo sát có kết quả kiểm chứng; không có placeholder cho chức năng đã đưa vào phạm vi. Hạn chế chưa kiểm chứng phải được báo rõ trong bàn giao.

## 13. Kế hoạch triển khai sau duyệt

| Giai đoạn | Kết quả phải hoàn thành |
|---|---|
| P1 | Scaffold workspace, UI shell, Tauri IPC, FFmpeg/FFprobe runtime và Windows build |
| P2 | Generic HLS engine, track selection, tải và ghép fixture thành video hợp lệ |
| P3 | SQLite, session/ticket, retry, bounded concurrency, pause/resume/cancel/recovery |
| P4 | Adapter Film4k, PNG-prefix transform, phụ đề và validation đầu ra |
| P5 | Hoàn thiện mọi màn hình, error/empty/loading states, settings và history |
| P6 | Integration/E2E, file lớn, installer, CI và tài liệu sử dụng/bàn giao |

Không ước lượng lịch cố định trước khi kiểm tra môi trường build. Khi bắt đầu code, ghi rõ dependency thiếu, phạm vi thay đổi và tiến độ theo kết quả nghiệm thu.

## 14. Các quyết định để chủ dự án duyệt

1. App Windows 10/11 x64, Tauri 2 + Rust + React; chưa làm webapp.
2. Hỗ trợ generic HLS VOD và adapter Film4k, không cam kết mọi website.
3. Một video + một audio + tối đa một subtitle/output; MP4 mặc định, MKV fallback phù hợp.
4. Soft subtitle hoặc SRT/VTT riêng; chưa burn-in/transcode.
5. Tải local, không cloud/account; pause/resume và recovery nằm trong v1.
6. Sau duyệt, triển khai đầy đủ P1–P6 và nghiệm thu theo SRS; thay đổi lớn cần cập nhật tài liệu.

## 15. Tài liệu kỹ thuật đã đối chiếu

- Tauri frontend/runtime: https://v2.tauri.app/start/frontend/
- Tauri architecture: https://v2.tauri.app/concept/architecture/
- reqwest Client reuse: https://docs.rs/reqwest/latest/reqwest/struct.Client.html
- reqwest cookie provider: https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html

**Lịch sử chỉnh sửa:** 1.0 — Chuyển đúng yêu cầu sang SRS, loại phần web SSR hiểu nhầm, xác định phạm vi Windows và tiêu chí nghiệm thu.

# Kết quả kiểm thử — 07/10/2026

## Đã xác nhận

- 14 test Rust và 11 test frontend qua; TypeScript typecheck, Rust fmt và Clippy toàn workspace (`-D warnings`) qua.
- Rust → TypeScript DTO khớp qua `pnpm contracts:check`.
- Pipeline thực sự tạo HLS bằng FFmpeg, phục vụ HTTP local, tải bằng engine rồi ghép video hoàn chỉnh. Không dùng mock cho media/HTTP/storage/FFmpeg.
- MPEG-TS có cookie bảo vệ, lỗi 503 lần đầu rồi retry, soft subtitle tiếng Việt trong MP4; ffprobe xác nhận subtitle language `vie`.
- Video fMP4 và audio ở hai playlist riêng, init map, ghép MKV, xuất SRT với offset +2 giây; cue 1–3 giây thành 3–5 giây.
- Pause → shutdown → mở SQLite lại → resume dùng checkpoint đã tải. Hủy tải dọn đúng cache job, không xuất video và không sửa file có sẵn.
- Parser: quoted commas, map/range/stable ID, chặn live/encryption, bỏ PNG tới IEND, chặn HTML/image không phải media, timestamp map và cue trùng.
- Session: cookie scope/path, yêu cầu HTTP 206 khi dùng range, single-flight refresh ticket. Error redaction ẩn signed URL.
- UI interaction tests: thêm/phân tích nguồn, chọn track và tạo job, lỗi inspect, pause/resume/cancel, settings, lịch sử và đóng ứng dụng. Browser inspection đã xem các trang chính; engine chạy qua native IPC trong bản desktop, browser preview chỉ xem UI.
- Adapter Film4k kiểm tra trực tiếp URL Spider-Man: metadata 8700.52 giây, 4 video / 4 audio / 39 subtitle; init và media trả HTTP 200, Content-Type image/png, PNG prefix 67 byte với fMP4 phía sau. Hai track Việt có 1777 và 1801 cue. Smoke này chỉ đọc đầu segment và phụ đề, không tải lại toàn bộ phim bằng app mới.
- Release executable khởi động trên máy hiện tại, còn chạy sau 5 giây và tạo SQLite; ffmpeg/ffprobe sidecar chạy được (9.0.2 essentials). Đã dừng process thử nghiệm và dùng data directory riêng.
- RAM process của pipeline test chạy tuần tự đạt đỉnh 17.47 MiB (lấy mẫu 50 ms). Native app process lúc khởi động khoảng 28.86 MiB. Số này không gồm WebView2/FFmpeg và không thay thế benchmark phim dài.
- Build NSIS Windows x64 thành công. Installer đóng gói ứng dụng, FFmpeg/ffprobe và notices; cấu hình tải WebView2 nếu thiếu.
- Cài silent vào thư mục thử riêng trên máy hiện tại thành công; bản đã cài khởi động, tạo SQLite và chứa cả hai sidecar cùng 3 file license. Gỡ bản thử thành công (exit 0). Không thay thế kiểm tra trên Windows sạch.

## Chưa xác nhận đầy đủ

- Chưa kiểm thử bộ cài trên Windows sạch, thiếu WebView2, hoặc qua mọi màn hình native sau khi cài. Không có chứng chỉ ký số.
- Chưa kiểm thử tải toàn bộ phim dài từ nguồn Film4k bằng release mới; nguồn được kiểm chứng bằng adapter smoke, pipeline đầy đủ dùng video fixture.
- Chưa làm fault injection disk-full và kill process ở từng stage muxing/verifying. Đã kiểm thử pause/shutdown/restart có checkpoint.
- Chưa benchmark RAM/network với phim nhiều giờ và concurrency 16 hoặc mất mạng kéo dài. Không có soak test.
- GitHub Actions workflow đã có nhưng chưa chạy trên GitHub vì dự án chưa được đưa lên repository remote.

## Đối chiếu SRS

| Nhóm | Trạng thái |
|---|---|
| FR-01…FR-16 | Đã triển khai; parser/session/pipeline/UI có test, nguồn Film4k có smoke |
| FR-17 | Lịch sử completed/failed/cancelled; retry job failed và resume paused |
| FR-18 | Command mở file/folder đã triển khai; thao tác mở bằng OS chưa test tự động |
| FR-19…FR-21 | Settings, diagnostics và cleanup đã triển khai; lỗi dùng mã chung DOWNLOAD_FAILED |
| FR-22 | Có installer và startup smoke trên máy hiện tại; nghiệm thu máy sạch còn thiếu |
| NFR-01…NFR-03 | Bounded worker/streaming; RAM fixture đã đo, phim dài chưa benchmark |
| NFR-04 | Recovery graceful đã test; crash injection mọi stage chưa test |
| NFR-05…NFR-07 | Cookie scope, redaction, process cancellation/path protections có kiểm tra |
| NFR-08 | Kiểm tra free space và propagation lỗi ghi; disk-full runtime chưa fault injection |
| NFR-09…NFR-10 | Progress coalesce ~250ms, modules độc lập, DTO check và migrations |
| NFR-11 | Lockfiles, CI và installer có; CI remote/máy sạch chưa thực thi |
| NFR-12 | Binary pin SHA-256, upstream GPLv3 notices/source/build info kèm theo; xem licenses |

Đây là bản triển khai 0.1.0 dùng thử cục bộ. Không đánh dấu hoàn tất mọi tiêu chí nghiệm thu v1 của SRS khi các kiểm tra nêu trên chưa được thực hiện.

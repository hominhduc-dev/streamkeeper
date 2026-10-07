# Streamkeeper

Ứng dụng Windows tải HLS VOD thành MP4/MKV, giữ session trong quá trình tải, ghép phụ đề và quản lý thư viện phim. Giao diện tiếng Việt. Phiên bản 0.3.0.

[Tải bộ cài từ GitHub Releases](https://github.com/hominhduc-dev/streamkeeper/releases/latest).

## Dùng ứng dụng

1. Chạy bộ cài trong `releases/`. Windows cần WebView2; bộ cài có thể tải thành phần này nếu chưa có.
2. Chọn **Thêm video**, dán URL trang xem Film4k hoặc URL HLS `.m3u8` trực tiếp, rồi phân tích.
3. Chọn chất lượng, audio, phụ đề và thư mục đích. Chọn MP4 để phát phổ biến hoặc MKV.
4. Phụ đề: **ghép vào video** tạo track bật/tắt trong player; **SRT/VTT** tạo file riêng. Offset dương làm phụ đề xuất hiện muộn hơn, âm làm sớm hơn.
5. Tạo tác vụ. Ứng dụng tải segment, ghép và kiểm tra video trước khi công bố file cuối cùng.
6. Có thể tạm dừng khi đang chờ/đang tải. Khi đóng ứng dụng, chọn lưu tiến độ; lần mở tiếp theo chọn tiếp tục.

Mặc định file nằm ở `Downloads/Streamkeeper`, cache và SQLite ở `%LOCALAPPDATA%/dev.streamkeeper.desktop`. Không ghi đè file đã có. Thư mục đích cần NTFS để công bố file bằng hard link, và đủ dung lượng cho cả cache lẫn video. Một tác vụ chạy tại một thời điểm, tối đa 16 request segment đồng thời (mặc định 8).

## Phạm vi hiện tại

**Chống sleep:** Cài đặt → “Giữ máy hoạt động khi đang tải” (mặc định bật). Windows power request giữ máy trong lúc downloading/muxing/verifying, màn hình vẫn được tự tắt. Pause, cancel, lỗi, hoàn tất, tắt tùy chọn hoặc thoát app sẽ giải phóng request. Cài đặt cũ tự nhận mặc định bật. Không thay đổi power plan. Không chặn Sleep thủ công hay đóng nắp; Modern Standby chạy pin có thể giới hạn request theo chính sách Windows. Chi tiết và kiểm thử trong [POWER.md](docs/POWER.md).

**Thư viện phim:** video hoàn tất tự xuất hiện, kể cả các job của bản cũ. Poster tạo và cache từ khung hình video. Tìm tên không dấu, lọc đã xem/chưa xem/file bị thiếu và MP4/MKV. Trạng thái đã xem lưu qua restart. Đổi tên cập nhật tên phim và file video thật, chặn tên sai/trùng. Xóa cần xác nhận và xóa file video khỏi ổ đĩa, bỏ khỏi thư viện; phụ đề rời và lịch sử giữ lại. Nếu file bị di chuyển/xóa bên ngoài, thư viện báo file bị thiếu. Không tự quét/import các video ngoài lịch sử tải của ứng dụng.

- Film4k: metadata, play ticket, cookie tự quản lý trong bộ nhớ, refresh ticket, bỏ PNG prefix theo cấu trúc chunk.
- HLS VOD: master/media playlist, MPEG-TS/fMP4, audio riêng, init map, byte range, signed URL, retry và checkpoint SHA-256.
- Phụ đề WebVTT đơn hoặc segmented, timestamp map, xuất SRT/VTT và soft subtitle MP4/MKV.
- Hàng đợi, lịch sử, thông báo lỗi, cấu hình, diagnostic đã ẩn URL.

Chưa hỗ trợ DRM, HLS mã hóa, live stream, đăng nhập trình duyệt hoặc nhập cookie từ Chrome, DASH, burn-in phụ đề và tự tìm phụ đề từ bên ngoài. Adapter Film4k phụ thuộc API nguồn; khi API đổi có thể cần cập nhật. URL nguồn được lưu cục bộ trong SQLite để resume; cookie và ticket không được lưu. Bộ cài chưa ký số và chưa được kiểm thử trên máy Windows sạch.

## Phát triển

Windows x64, Node.js 22+, pnpm 11.19.0, Rust stable MSVC, Visual Studio Build Tools với C++ workload và Windows SDK, WebView2.

```powershell
pnpm install --frozen-lockfile
powershell -ExecutionPolicy Bypass -File scripts/prepare-tools.ps1
pnpm tauri dev
```

`pnpm dev` chỉ xem giao diện trong trình duyệt; engine tải chạy trong Tauri.

Gói source ZIP đã kèm hai binary media đã kiểm tra, nên có thể bỏ bước `prepare-tools.ps1` khi dùng ZIP. Script dành cho checkout không chứa binary; nếu upstream đổi gói release, checksum sẽ từ chối bản chưa kiểm tra.

```powershell
pnpm typecheck
pnpm test
cargo test -p video-domain -p video-media -p video-providers -p video-storage -p video-engine
cargo fmt --all --check
pnpm contracts
pnpm tauri build
```

Bộ cài sinh tại `target/release/bundle/nsis/`. Các test pipeline tạo video mẫu bằng FFmpeg và server HTTP cục bộ, không tải phim. Xem [kiến trúc](docs/ARCHITECTURE.md), [kết quả kiểm thử](docs/VALIDATION.md), [SRS](docs/SRS.md) và [thành phần bên thứ ba](docs/licenses/THIRD-PARTY.md).

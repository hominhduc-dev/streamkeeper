# Benchmark RAM/network và soak test

Bài test opt-in dùng engine thật, SQLite/checkpoint thật và FFmpeg đóng gói. HTTP server chỉ nghe loopback, không ngắt mạng Windows hoặc tải phim từ bên ngoài. Dữ liệu được tạo trong thư mục tạm độc lập và tự dọn khi kết thúc.

## Chạy trên Windows

Cài Python `psutil` vào môi trường riêng; đặt Cargo/Rust và Python trên PATH. Từ thư mục repository:

```powershell
python scripts/soak.py --report-dir .local/soak
```

`--quick` chỉ kiểm tra harness: nội dung 20 phút, mất nguồn 45 giây. Không dùng kết quả quick để khẳng định đã chạy phim nhiều giờ.

Mặc định: video tổng hợp 3 giờ, HLS 10 giây/segment, 16 luồng; mỗi segment thêm khoảng 2 MiB MPEG-TS null packets hợp lệ để tạo tải network/disk. Máy chủ gửi chunk 64 KiB với khoảng nghỉ 50 ms. Sau ít nhất 32 checkpoint, nguồn trả HTTP 503 liên tục 180 giây. Kiểm tra engine hết retry, giữ cache, không công bố output, rồi resume khi nguồn trở lại. Mọi segment đã checkpoint phải được tái sử dụng mà không request lại. Cuối cùng engine kiểm tra thời lượng, track hình/tiếng, toàn bộ packet và giải mã mẫu trước khi hoàn tất.

Các biến môi trường:

| Biến | Mặc định |
| --- | ---: |
| SOAK_MEDIA_SECONDS | 10800 |
| SOAK_OUTAGE_SECONDS | 180 |
| SOAK_SEGMENT_BYTES | 2097152 |
| SOAK_CHUNK_MS | 50 |
| SOAK_TIMEOUT_SECONDS | 1800 |

Cần khoảng 3 GiB dung lượng trống với cấu hình mặc định. Fixture màu và âm sin không đại diện độ phức tạp giải mã phim 4K HEVC; nó kiểm tra đường tải, checkpoint và ghép VOD dài.

## Kết quả và giới hạn

Ngưỡng RSS mặc định của runner là 256 MiB cho engine cùng server; vượt ngưỡng sẽ trả exit code lỗi. Có thể điều chỉnh bằng `--max-rss-mib`. Ngưỡng này dành cho fixture kiểm thử, không phải giới hạn RAM áp đặt lên ứng dụng.

`soak.json` có mẫu RSS/private memory mỗi giây, peak RSS của test process và tiến trình media con, thời gian chạy, số byte server gửi, số request, HTTP 503, peak số response đang stream và số segment dùng lại. `soak.log` chứa trạng thái và assertion.

RAM test process bao gồm engine **và server giả lập**, không bao gồm Tauri/WebView UI. Server dùng padding chia sẻ và stream chunk; không giữ cả phim trong RAM. Byte network là payload qua loopback, không phải tổng overhead TCP/TLS hay tốc độ Internet. HTTP 503 mô phỏng nguồn mất khả dụng; không tương đương rớt Wi-Fi, DNS hoặc TCP blackhole.

Test thường `cancel_sixteen_stalled_connections_without_waiting_for_network_timeout` mở 16 request mà server không trả header trong 180 giây; hủy tác vụ phải xong trong 5 giây và dọn cache an toàn. Test này xác minh hủy request đang treo, không chờ đủ 180 giây.

Một lượt mặc định có nội dung 3 giờ nhưng thời gian tải thực tế chỉ vài phút. Đây là soak có giới hạn, **không phải soak 8–24 giờ** và chưa chứng minh không rò bộ nhớ trong nhiều ngày. Muốn chạy lâu hơn, tăng số giây nội dung và thời gian chờ chunk, đồng thời tăng timeout và chuẩn bị đủ đĩa.

Không tự tiếp tục khi mất nguồn quá giới hạn retry: tác vụ chuyển `failed`; người dùng phải bấm tiếp tục sau khi mạng phục hồi. Benchmark chỉ gọi cùng API resume đó.

## Lượt kiểm chứng ngày 08/10/2026

Trên Windows local: nguồn 3 giờ / 1.080 segment, concurrency 16, outage HTTP 503 dài 180 giây. Truyền 2.293.997.620 byte (~2,14 GiB); tổng 342,55 giây gồm tạo fixture. Peak RSS engine cùng server 29,1 MiB; peak tổng RSS media child 28,0 MiB. File MP4 cuối hợp lệ. Các null packets tạo tải truyền được loại bỏ khi ghép nên MP4 cuối chỉ khoảng 22,4 MB.

Lượt quick kiểm tra assertion checkpoint SQLite: 20 phút / 120 segment, outage 45 giây. Có 64 checkpoint và cả 64 được dùng lại, không request thêm; RAM dưới ngưỡng 256 MiB. Test hủy 16 kết nối đang treo mất 0,063 giây. Không có thay đổi production trong đợt này.

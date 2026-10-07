# Giữ máy hoạt động — 0.3.0

Tùy chọn mặc định bật, lưu `preventSleep` trong Settings JSON. Serde default đảm bảo cài đặt của bản cũ không lỗi và nhận mặc định bật. Không cần migration DB hoặc chỉnh power plan của người dùng.

Engine dùng PowerCreateRequest/PowerSetRequest với PowerRequestSystemRequired và reason tiếng Việt. OwnedHandle trong Mutex cho phép request đi qua các Tokio worker mà không phụ thuộc thread. Không dùng DisplayRequired nên màn hình được tắt bình thường. Refresh ở mỗi chuyển trạng thái job và thay đổi settings; chỉ downloading/muxing/verifying cần giữ máy, queued/paused/failed/cancelled/completed không cần. Shutdown giải phóng ngay. Drop đóng handle; Windows giải phóng object khi process kết thúc. Lỗi API không làm hỏng tải, có thể xem trong Cài đặt qua runtime status.

Tùy chọn chỉ chống sleep do không hoạt động: Sleep thủ công, nút nguồn/đóng nắp và chính sách nguồn của Windows vẫn có hiệu lực. Modern Standby trên pin có thể chấm dứt yêu cầu sau thời hạn do Windows quy định. Tham khảo [Microsoft PowerSetRequest](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest).

Kiểm thử: gọi Windows API tạo/giải phóng request; lifecycle cho các trạng thái và shutdown/tắt setting; settings cũ thiếu preventSleep; pipeline bật/tắt setting khi đang tải, pause/restart/complete; UI lưu tùy chọn. Không đưa máy thật vào sleep vì sẽ ngắt phiên làm việc. Không kiểm thử đóng nắp hay Modern Standby trên pin.

Cùng bản này sửa nghẽn SQLite khi ghi progress: các future segment trước đây chỉ tiến triển khi buffer_unordered được poll; progress update chờ DB có thể giữ việc poll các checkpoint đang nắm connection, gây pool timeout trên CI chậm. Segment chuyển sang JoinSet có giới hạn, tự được poll trong lúc progress ghi DB. Lỗi/cancel abort và drain tasks trước khi dọn cache, không để worker chạy tách rời. Fixture phản hồi 350ms để vượt ngưỡng progress 250ms và kiểm tra contention.

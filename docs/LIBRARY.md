# Thư viện phim — 0.2.0

Mở **Thư viện phim** trên sidebar. Tất cả job completed có output tự xuất hiện, kể cả dữ liệu từ bản 0.1.0. Không quét video bên ngoài ứng dụng.

- Poster tự tạo từ khung hình ở 10% thời lượng (tối đa giây 60), cache JPEG trong app data. Đây là ảnh từ video, không phải poster quảng bá tải từ Internet.
- Tìm tên không phân biệt hoa/thường hoặc dấu tiếng Việt. Lọc tất cả/đã xem/chưa xem/file bị thiếu, và MP4/MKV.
- **Đánh dấu đã xem/chưa xem** lưu trong SQLite và giữ sau restart.
- **Đổi tên** cập nhật tên hiển thị, output path và file video thật. Không ghi đè, không cho tên/path sai. Thư mục NTFS cần quyền ghi. Phụ đề rời giữ nguyên tên.
- **Xóa** hiện xác nhận trước khi xóa file video thật và bỏ khỏi thư viện. Phụ đề rời và lịch sử tải giữ nguyên. Xóa vĩnh viễn, không đưa vào Recycle Bin. File đang bị player khóa sẽ báo lỗi để đóng player rồi thử lại.
- File bị di chuyển/xóa bên ngoài được đánh dấu **Không tìm thấy file**, tắt nút xem/đổi tên; có thể xóa mục thiếu khỏi thư viện.

Migration `0002_library.sql` thêm bảng watched/deleted theo job ID; không sửa migration cũ và không reset job/history. Các command thư viện chỉ nhận job ID trong ứng dụng, không nhận đường dẫn file tùy ý để xóa. Rename dùng hard link không ghi đè và cập nhật job, giữ poster/watched theo cùng ID.

Kiểm tra local: 14 Rust tests và 14 frontend tests qua. Pipeline thật tạo poster JPEG, chặn rename tên sai/trùng, đổi tên file, persist trạng thái watched và title qua reopen SQLite, rồi xóa video mà không sửa file có sẵn khác. UI tests kiểm tra search/filter, watched/rename commands và chỉ xóa sau xác nhận. Typecheck, Clippy và contract check qua.

Bản 0.1.0 trên GitHub Actions gặp lỗi timeout pool SQLite khi ghi song song. 0.2.0 dùng một connection để tuần tự hóa writer SQLite; kết quả CI mới được kiểm tra riêng khi đẩy lên GitHub. Bản release vẫn cần nghiệm thu native UI trên Windows sạch như đã ghi trong VALIDATION.md.

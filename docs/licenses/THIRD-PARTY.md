# Thành phần bên thứ ba

FFmpeg và ffprobe được phân phối dưới dạng executable sidecar độc lập; Streamkeeper gọi bằng process arguments, không liên kết thư viện FFmpeg.

- Nhà cung cấp binary: https://www.gyan.dev/ffmpeg/builds/
- Gói: ffmpeg-release-essentials.zip; version hiện tại ghi trong README.txt kèm theo.
- Giấy phép binary: GPL v3, bản LICENSE nằm cùng thư mục này.
- Source revision của FFmpeg: https://github.com/FFmpeg/FFmpeg/commit/946fcce07b
- Build configuration và thông tin source: README.txt gốc từ gói Gyan. Gói upstream có thư viện phụ thuộc; giữ nguyên notices upstream khi phân phối lại.

Script prepare-tools tải gói upstream và kiểm tra SHA-256 đã pin. Binary không nằm trong Git. Khi đổi bản FFmpeg, phải cập nhật checksum, tài liệu license, source revision và chạy lại test pipeline trước khi phát hành. Bộ cài đóng gói cả thư mục license để người nhận đọc được.

React, Tauri và các thư viện JavaScript/Rust còn lại dùng giấy phép trong metadata/package tương ứng. pnpm-lock.yaml và Cargo.lock ghi chính xác dependency versions; cần rà soát giấy phép toàn bộ dependency trước phát hành thương mại rộng rãi.

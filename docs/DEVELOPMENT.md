# Phát triển và kiểm chứng

## Cấu trúc

- `Sources/Config/EnvironmentConfig.swift`: chọn thư mục dữ liệu, đọc runtime/environment ID, xây URL.
- `Sources/Data/DatabaseManager.swift`: actor sở hữu SQLite read-only và date formatter, trả dữ liệu `Sendable` hoặc lỗi rõ ràng.
- `Sources/Data/ThreadModel.swift`: trạng thái từ projection đã lưu.
- `Sources/Data/ThreadMonitor.swift`: một lần đọc bất đồng bộ tại một thời điểm; tổng hợp badge từ toàn bộ inbox, độc lập bộ lọc tìm kiếm.
- `Sources/Views/PopoverView.swift`: danh sách và thông báo loading/offline/lỗi; khi dữ liệu cũ thì không hiện nhãn live.
- `Sources/Utils/AppLauncher.swift`: mở ứng dụng/link qua macOS và báo lỗi khi không mở được.
- `Tests/`: fixture SQLite riêng trong thư mục tạm, không dùng dữ liệu cá nhân hoặc server thật.

## Kiểm tra tự động

Chạy từ thư mục gốc:

```bash
swift test
swift build -c release
bash -n scripts/build_app.sh scripts/install.sh
```

Test bao phủ inbox hơn 50 thread, thứ tự pin, thread ẩn, silent run/cancelled/queued, mức ưu tiên trạng thái, Done cho completion chưa xem, cập nhật visit và completion mới, thiếu/invalid visit, dữ liệu offline/lỗi và phục hồi, tìm kiếm không làm mất badge tổng, thư mục dữ liệu tùy chỉnh, port runtime, encoding URL và thiếu cấu hình. Bổ sung regression Snooze: approval/input đang chờ, completion/lỗi phiên trước–bằng–sau snooze, request đã giải quyết, Woke sau lần xem và mốc ngày không hợp lệ.

Mỗi lệnh kiểm tra phải trả mã 0; lỗi build/test/cú pháp trả mã khác 0. Với Swift Testing, xác nhận dòng cuối có số test thực sự chạy và passed; phần XCTest có thể báo 0 vì project dùng Swift Testing.

CI chỉ chạy test/build và kiểm tra cú pháp script. Không upload `.app`, ký Developer ID, notarize, commit hay publish tự động.

## Kết quả local ngày 2026-11-04

- `swift test`: 28 test trong 3 suite đều qua, bao gồm lock SQLite, lỗi JSON khi đọc kết quả và chuyển Done → thời gian sau khi T3 Code lưu lần xem mới.
- `swift build -c release`: thành công; cảnh báo API activation deprecated trước đó đã được loại bỏ.
- `bash -n scripts/build_app.sh scripts/install.sh`: qua. Đã đóng gói và mở `.app` local theo yêu cầu xem thử; không phát hành artifact tải về.
- Cài bằng `scripts/install.sh` vào thư mục tạm với `T3_CODE_BAR_SKIP_OPEN=1`: thành công; resource bundle và license/notices có đủ. Chạy lại bị từ chối với mã khác 0 và không thay đổi binary cũ. Chưa kiểm tra nhánh tự mở app của installer mới.
- Các link Markdown local: hợp lệ. README sử dụng ảnh PNG do tác giả cung cấp, giữ nguyên ảnh gốc, không cắt phần menu bar hệ thống; không dùng hình minh họa SVG trước đó. Cú pháp workflow YAML và quy tắc `.gitignore` đã được kiểm tra ở đợt trước.
- Truy vấn SQL hiện tại đọc thành công projection database local qua kết nối read-only; không in title/nội dung thread. Đây là kiểm tra tương thích truy vấn, không phải benchmark hay UAT toàn bộ ứng dụng.
- Binary Release mới: arm64, linker-signed ad hoc; không có Developer ID. Không thực hiện notarization.
- Người dùng xác nhận approval/input hiển thị đúng. Chưa xác nhận UAT thủ công riêng cho tình huống đang Snooze; chưa chạy workflow trên GitHub hoặc hoàn tất toàn bộ checklist bên dưới.

## Kiểm thử thủ công trước khi chia sẻ

1. Chạy T3 Code rồi `swift run T3CodeBar`; xác nhận biểu tượng menu bar xuất hiện.
2. Tạo lượt chạy, yêu cầu approval và user input; đối chiếu thread/nhãn với T3 Code. Kiểm tra thêm thread đang snooze được đánh thức khi có yêu cầu hoặc completion mới; xem thread trong T3 Code và kiểm tra Woke theo dữ liệu visit đã lưu.
3. Click thread, Copy Link và Open in Browser; thử instance dùng port khác nếu có.
4. Thoát T3 Code: banner Offline hiện và badge hoạt động tắt. Mở lại: dữ liệu cập nhật trở lại.
5. Dùng `T3_CODE_BAR_STATE_DIR` trỏ tới một thư mục tạm không có database: xác nhận báo thiếu dữ liệu; không sửa/xóa database thật để thử lỗi.
6. Cuộn danh sách dài, kiểm tra title/branch/provider và click phải biểu tượng để thoát.
7. Nếu dùng script đóng gói local, mở app từ Finder ở ngoài thư mục source để kiểm tra resource bundle.

Các bước này là checklist, không phải tuyên bố đã hoàn tất UAT. Chưa có kiểm chứng Intel, Gatekeeper hay notarization. Chưa có benchmark RAM/CPU/độ trễ.

## Trước commit đầu tiên

Trước mỗi commit, kiểm tra danh sách staged để chỉ đưa source, tài nguyên cần thiết, test, script, workflow, README/docs và giấy phép. `.build/`, `build/`, `.local-reference/`, database và file môi trường được loại bởi `.gitignore`.

Không đưa ảnh chụp có tên thread, đường dẫn dự án hoặc nội dung cá nhân lên public nếu chưa làm sạch dữ liệu.

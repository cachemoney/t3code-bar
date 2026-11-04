# Tương thích với T3 Code

Đối chiếu ngày 2026-11-04:

- Source tham khảo: repository `pingdotgg/t3code`, revision `21a3669cee0c6fb54f5afe065cf0eb0348bd232c`.
- Các nguồn tham khảo chính: `apps/web/src/components/Sidebar.logic.ts`, `packages/client-runtime/src/state/threadSort.ts`, `packages/client-runtime/src/state/threadSettled.ts`, `apps/server/src/orchestration/ThreadBackgroundLiveness.ts`, `apps/server/src/config.ts`.
- Runtime local được kiểm tra metadata: `0.1.7-nightly.20261104.4151`.
- Runtime đã cài và source tham khảo không được coi là cùng một revision: schema projection v2 được xác minh trực tiếp từ database local; không suy ra chứng nhận tương thích với mọi build T3 Code từ source trên.

## Hợp đồng SQLite

App yêu cầu các bảng sau, với các cột được truy vấn trong `Sources/Data/DatabaseManager.swift`:

- `orchestration_v2_projection_threads`
- `orchestration_v2_projection_runs`
- `orchestration_v2_projection_runtime_requests`
- `orchestration_v2_projection_plans`
- `orchestration_v2_projection_provider_threads`
- `orchestration_v2_projection_messages`
- `projection_projects`
- `projection_thread_pull_requests`

Các thuộc tính inbox như `settledAt`, `snoozedAt`, `snoozedUntil`, `pinnedAt`, `pinOrderKey`, `activeOrderKey`, `lineage` và `lastVisitedAt` được đọc từ JSON payload. Không sửa schema, tạo index hay migrate dữ liệu của T3 Code. Khi SQL không chuẩn bị được vì thiếu bảng/cột, app báo lỗi tương thích thay vì hiển thị inbox trống.

App mở SQLite read-only; không dùng `immutable=1`, để vẫn đọc thay đổi trong WAL. Mỗi lượt đọc mở và đóng kết nối; chờ lock tối đa 250 ms. Lỗi khi bước qua kết quả không được coi là đọc thành công một phần.

## Snooze và Woke

Snooze được phân loại sau khi đọc trạng thái, không loại thread bằng SQL chỉ dựa vào giờ hẹn. Approval/input pending luôn vượt snooze; completion của lượt hiển thị sau `snoozedAt` hoặc lỗi phiên mới cũng đánh thức sớm. Phiên được lấy từ payload provider-thread có `last_run_ordinal` lớn nhất (`status`, `updatedAt`); đây là ánh xạ projection-v2 local cho dữ liệu session của source tham khảo. Không suy đoán lỗi mới từ `thread.updated_at`.

Woke dùng mốc completion nếu completion đánh thức sớm, nếu không dùng thời gian session/snooze cho sự kiện cần chú ý, hoặc giờ hẹn cho timer wake. So sánh với `lastVisitedAt` đã persistence để xác định còn chưa xem. Ngày snooze không hợp lệ không làm ẩn thread. Bar không sửa snooze/visit và không gọi API acknowledge riêng của sidebar; chỉ phản ánh dữ liệu đã lưu.

## Những gì không thể suy ra từ SQLite này

`ThreadBackgroundLivenessService` của source tham khảo dùng registry trong bộ nhớ, không persistence. Nó phân biệt agent/workflow đang chạy với vòng watch chỉ theo dõi. App hiện không kết nối tới runtime service này và không suy đoán trạng thái từ task cũ trong lịch sử.

Source tham khảo dùng `lastVisitedAt` trong state của client. Bản nightly local đã cài còn persistence trường này vào payload của `orchestration_v2_projection_threads`; app đọc trường đó trực tiếp. Done chỉ hiện nếu lượt hiển thị gần nhất có status completed và `completedAt > lastVisitedAt` hợp lệ. Đã xem, bằng mốc hoàn thành hoặc thiếu mốc đã xem hợp lệ thì hiện thời gian. Khi T3 Code lưu lần xem mới, lần poll tiếp theo của Bar tự cập nhật; Bar không tự ghi mốc đã xem khi người dùng click link. Những bản chỉ lưu visit trong localStorage mà không persistence vào projection sẽ không có chỉ báo Done này. T3 Code Bar vẫn giữ các nhãn riêng Goal, Failed, Woke và không tuyên bố giống hoàn toàn sidebar.

Nguồn dữ liệu cố định cho một instance local. Không ghép remote environment, không tự phát hiện tất cả instance, không truy cập token hoặc kho credential.

## Kiến trúc và phân phối

- macOS deployment target: 14.0.
- Kiến trúc đã kiểm tra local: arm64.
- x86_64 và universal binary: chưa kiểm tra.
- Binary local: linker-signed ad hoc, không Developer ID, chưa notarize.
- `.app` sẵn có trong `build/` là output local, có thể cũ hơn source; không phải artifact phát hành và không được commit.

Việc build thành công không thay thế kiểm thử thao tác người dùng, Gatekeeper hoặc xác nhận trên máy khác.

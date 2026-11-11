# Đặc tả Tính năng: Nâng cấp GridCraft v0.18.0 VN & Copy Nhiều Ô dạng Ảnh + Text (Chuẩn Excel/Zalo)

**Feature ID**: `001-v0180-clipboard-image-copy`  
**Status**: Draft — Ready for Review  
**Date**: 2026-11-11  

---

## 1. Bối cảnh & Mục tiêu (Overview)

1. **Nâng cấp phiên bản mới nhất (`v0.18.0`) giữ nguyên tính năng Việt Nam (`vn-edition`)**:
   - Upstream `storytold/gridcraft` đã phát hành bản mới nhất `v0.18.0` (bao gồm cả `v0.17.0` và `v0.18.0`), khiến bản `0.16.1` hiện tại liên tục báo có bản cập nhật.
   - Cần hoàn tất đồng bộ mã nguồn `v0.18.0` vào nhánh `vn-edition` mà vẫn bảo toàn 100% các tùy biến cho người dùng Việt Nam.
2. **Copy vùng nhiều ô ra Clipboard dạng Ảnh + Text (Excel / Zalo Parity)**:
   - Hiện tại khi copy nhiều ô trong GridCraft rồi dán (`Cmd+V`) vào Zalo (hoặc các ứng dụng nhắn tin như Telegram, Messenger), clipboard chỉ chứa `HTML` và `Plain Text (TSV)` nên Zalo chỉ dán ra chuỗi văn bản thô, mất khung bảng và định dạng.
   - Khi copy từ Microsoft Excel dán vào Zalo, Excel ghi đồng thời cả **hình ảnh bảng tính (`PNG`)**, **bảng `HTML`** và **văn bản `TSV`** vào clipboard hệ thống. Nhờ đó Zalo mặc định hiển thị bản xem trước dạng **hình ảnh bảng tính rõ nét**, đồng thời cho phép người dùng chọn **"Dán dạng văn bản"** nếu muốn.

---

## 2. Câu chuyện Người dùng (User Stories)

### US1 (`P1` - MVP): Sao chép nhiều ô từ GridCraft dán vào Zalo hiển thị dạng Ảnh kèm tùy chọn Text
- **Mô tả**: Là người lập dự toán / kế toán sử dụng GridCraft trên macOS, khi tôi quét chọn nhiều ô và bấm `Cmd+C` (hoặc Copy) rồi sang Zalo bấm `Cmd+V`, tôi muốn Zalo nhận diện được hình ảnh bảng tính sắc nét (có kẻ khung, màu nền, chữ tiếng Việt, căn lề, in đậm) và đồng thời vẫn có tùy chọn chuyển sang gửi dạng văn bản thuần giống hệt khi copy từ Excel.
- **Kịch bản nghiệm thu (Acceptance Scenarios)**:
  - **Given** người dùng mở bảng tính trong GridCraft và chọn một vùng gồm từ 2 ô trở lên (`range.start != range.end`),
  - **When** người dùng thực hiện lệnh Copy (`Cmd+C`) hoặc Cut (`Cmd+X`),
  - **Then** clipboard hệ thống (`NSPasteboard`) được ghi đồng thời 3 định dạng: `public.png` (ảnh PNG của bảng), `public.html` (bảng HTML), và `public.utf8-plain-text` (văn bản TSV).
  - **Given** clipboard đang chứa dữ liệu vừa copy từ vùng nhiều ô của GridCraft,
  - **When** người dùng bấm `Cmd+V` trong cửa sổ chat Zalo,
  - **Then** Zalo hiển thị ảnh chụp vùng bảng tính với đầy đủ đường lưới, màu sắc, nội dung tiếng Việt rõ nét và cung cấp tùy chọn gửi dạng văn bản.

### US2 (`P1` - MVP): Giữ nguyên hành vi dán (`Cmd+V`) trong nội bộ GridCraft và khi copy 1 ô đơn lẻ
- **Mô tả**: Việc bổ sung định dạng ảnh PNG vào clipboard khi copy nhiều ô không được làm thay đổi hành vi dán (`Cmd+V`) bên trong GridCraft (vẫn dán dữ liệu/công thức/định dạng vào các ô thay vì dán đè thành một tấm ảnh nổi), và khi chỉ copy 1 ô đơn lẻ thì chỉ copy dạng text/HTML để dán nhanh vào các ô nhập liệu.
- **Kịch bản nghiệm thu (Acceptance Scenarios)**:
  - **Given** người dùng copy một vùng nhiều ô trong GridCraft,
  - **When** người dùng chọn một ô khác trong GridCraft và bấm `Cmd+V`,
  - **Then** GridCraft dán dữ liệu/công thức vào các ô bảng tính như bình thường (không chèn thành floating picture).
  - **Given** người dùng chỉ chọn đúng 1 ô đơn lẻ (`range.start == range.end`) và bấm `Cmd+C`,
  - **When** người dùng dán vào Zalo hoặc trình soạn thảo,
  - **Then** nội dung được dán ngay dưới dạng văn bản thuần (không tạo ảnh cho 1 ô đơn lẻ).

### US3 (`P1`): Nâng cấp lên GridCraft `v0.18.0` VN và tắt cảnh báo đòi Update
- **Mô tả**: Đồng bộ toàn bộ tính năng mới của `v0.17.0` – `v0.18.0` từ upstream vào nhánh `vn-edition`, giữ nguyên toàn bộ tùy biến tiếng Việt và đóng gói cài đặt bản `0.18.0` để ứng dụng không còn hiện thông báo đòi cập nhật.
- **Kịch bản nghiệm thu (Acceptance Scenarios)**:
  - **Given** bản cài đặt `/Applications/GridCraft.app` và `/opt/homebrew/bin/gridcraft-cli` được nâng cấp lên `0.18.0`,
  - **When** mở ứng dụng GridCraft,
  - **Then** ứng dụng hiển thị phiên bản `0.18.0`, không còn hiện banner báo có bản cập nhật mới, và giữ nguyên định dạng số Việt Nam (`1.234,56`), ngày `dd/mm/yyyy`, font `Times New Roman 13`, khổ giấy `A4`, cùng khả năng hiển thị tiếng Việt có dấu đầy đủ.

---

## 3. Các Trường hợp Biên (Edge Cases)

- **Chọn vùng quá lớn (ví dụ `Cmd+A` toàn bộ 1 triệu dòng)**: Bộ kết xuất ảnh clipboard sẽ tự động giới hạn vùng vẽ ảnh tối đa (ví dụ tối đa 100 dòng × 30 cột hoặc kích thước ảnh tối đa `4096 × 4096` px) để thao tác `Cmd+C` luôn phản hồi tức thì (< 100ms), trong khi dữ liệu text/HTML vẫn hoạt động bình thường.
- **Ô gộp (Merged cells), màu nền, viền ô và căn lề**: Ảnh PNG tạo ra phải thể hiện đúng ô gộp, màu nền (fill color), màu chữ, in đậm (bold), căn trái/phải/giữa và đường kẻ lưới mặc định (gridlines) giống giao diện bảng tính.
- **Văn bản tiếng Việt có dấu**: Sử dụng bộ font hệ thống đã nạp của `ui-egui` (hoặc font fallback tiếng Việt) để kết xuất chữ tiếng Việt sắc nét ở độ phân giải Retina (2x scale), không bị lỗi ô vuông `□`.

---

## 4. Yêu cầu Chức năng (Functional Requirements)

- **`FR-001`**: Hoàn tất merge nhánh `upstream/main` (`v0.18.0`) vào `vn-edition`, giữ nguyên toàn bộ tính năng custom Việt Nam.
- **`FR-002`**: Xây dựng hàm kết xuất vùng ô được chọn (`RangeRef`) thành ảnh bitmap RGBA / PNG sắc nét (hỗ trợ đường lưới, viền ô, màu nền, ô gộp, căn lề trái/giữa/phải, chữ đậm/nghiêng, màu chữ và tiếng Việt Unicode).
- **`FR-003`**: Mở rộng `Services` trong `crates/ui-egui` và triển khai trên macOS (`apps/gridcraft/src/main.rs`) sử dụng `objc2-app-kit` (`NSPasteboard`) để ghi đồng thời `public.html`, `public.utf8-plain-text`, và `public.png` vào cùng một pasteboard item khi sao chép vùng có nhiều hơn 1 ô.
- **`FR-004`**: Đảm bảo khi dán (`Cmd+V`) ngược lại vào GridCraft, ứng dụng luôn ưu tiên đọc dữ liệu bảng tính nội bộ / text / HTML thay vì dán ảnh PNG của chính lần copy đó.
- **`FR-005`**: Biên dịch bản Release `v0.18.0`, cài đặt vào `/Applications/GridCraft.app` và `/opt/homebrew/bin/gridcraft-cli`, ký mã số (`codesign`) và đẩy (push) lên kho lưu trữ `dinhsonconstruction/gridcraft-vn`.

---

## 5. Tiêu chí Thành công (Success Criteria)

- **`SC-001`**: Khi copy vùng $\ge 2$ ô trong GridCraft trên macOS, kiểm tra `NSPasteboard::generalPasteboard()` có mặt đồng thời cả 3 kiểu dữ liệu: `public.png`, `public.html`, và `public.utf8-plain-text`.
- **`SC-002`**: Dán vào Zalo hiển thị ngay ảnh bảng tính sắc nét kèm tùy chọn chuyển sang dạng văn bản.
- **`SC-003`**: Toàn bộ workspace vượt qua `cargo check --workspace` và các bài test liên quan với 0 lỗi biên dịch.
- **`SC-004`**: Ứng dụng `/Applications/GridCraft.app` chạy ở phiên bản `0.18.0`, không còn thông báo đòi update.

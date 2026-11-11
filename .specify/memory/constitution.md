# Hiến pháp Dự án GridCraft VN (Project Constitution)

**Version**: 1.0.0  
**Ratified**: 2026-11-11  
**Last Amended**: 2026-11-11  

## 1. Các Nguyên tắc Cốt lõi (Core Principles)

### I. Bảo toàn Chuẩn Văn hóa Excel Việt Nam (`vn-edition`)
- Mọi bản đồng bộ từ upstream (`storytold/gridcraft`) **PHẢI (MUST)** giữ nguyên 100% các tùy biến bản địa hóa cho kế toán, dự toán và xây dựng Việt Nam:
  - Phân cách hàng nghìn bằng dấu chấm (`.`), phân cách thập phân bằng dấu phẩy (`,`), phân cách đối số hàm bằng dấu chấm phẩy (`;`).
  - Định dạng ngày tháng mặc định `dd/mm/yyyy`.
  - Font chữ mặc định **Times New Roman**, cỡ chữ **13**, khổ giấy in mặc định **A4**.
  - Hệ thống fallback font tiếng Việt đầy đủ (`Times New Roman`, `Arial`, `.Vn*`) chống lỗi ô vuông `□` và giữ nguyên nét in đậm (Bold) khi hiển thị và xuất PDF/in ấn.
  - Tính năng Unhide hàng/cột/sheet chuẩn Excel kèm khôi phục kích thước cột/hàng bị kéo hẹp (sliver restore).

### II. An toàn Bộ nhớ & Không dùng `unsafe` trực tiếp (`unsafe_code = "forbid"`)
- Toàn bộ các crate trong workspace **PHẢI (MUST)** tuân thủ `#![forbid(unsafe_code)]` (hoặc cấu hình lints của workspace).
- Mọi tương tác với macOS AppKit / Pasteboard (`NSPasteboard`, `NSData`, `NSString`) **PHẢI (MUST)** sử dụng API an toàn của `objc2`, `objc2-app-kit` và `objc2-foundation` đã được ghim phiên bản trong `Cargo.lock`.

### III. Tương thích Đa định dạng Clipboard (Excel & Zalo Parity)
- Khi sao chép vùng nhiều ô (multi-cell range), hệ thống clipboard **PHẢI (MUST)** cung cấp đồng thời cả 3 định dạng dữ liệu trong cùng một lần ghi:
  1. `public.png` (Hình ảnh bảng tính sắc nét có đường kẻ, màu nền, định dạng chữ).
  2. `public.html` (Bảng HTML giữ nguyên cấu trúc và định dạng).
  3. `public.utf8-plain-text` (Văn bản phân cách bằng tab - TSV).
- Việc bổ sung ảnh vào clipboard **KHÔNG ĐƯỢC (MUST NOT)** làm hỏng hành vi dán (`Cmd+V`) ngược lại vào trong GridCraft hoặc các trình soạn thảo văn bản.

### IV. Giới hạn Tài nguyên & Kiểm thử Nghiêm ngặt
- Việc kết xuất ảnh vùng chọn vào clipboard **PHẢI (MUST)** giới hạn kích thước tối đa (số ô, chiều rộng/cao pixel) để không gây treo giao diện khi người dùng chọn toàn bộ bảng tính (`Cmd+A`).
- Mọi thay đổi **PHẢI (MUST)** vượt qua `cargo check --workspace` và các bộ unit test liên quan trước khi đóng gói phát hành vào `/Applications/GridCraft.app` và `/opt/homebrew/bin/gridcraft-cli`.

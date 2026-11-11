# Kế hoạch Kỹ thuật: Nâng cấp GridCraft v0.18.0 & Copy nhiều ô dạng Ảnh + Text (Chuẩn Excel/Zalo)

**Nhánh**: `vn-edition` | **Ngày**: 2026-11-11 | **Spec**: [spec.md](file:///Users/pain/.gemini/antigravity/scratch/gridcraft/specs/001-v0180-clipboard-image-copy/spec.md)

## 1. Tóm tắt (Summary)

1. **Hoàn tất đồng bộ Upstream `v0.18.0`**: Chốt merge từ `upstream/main` (`v0.18.0`) vào nhánh `vn-edition`, giữ nguyên 100% các tùy chỉnh bản Việt hóa (`vn-edition`: dấu phân cách số `.`/`,`, ngày `dd/mm/yyyy`, font mặc định `Times New Roman` 13pt, khổ A4, sửa lỗi hiển thị font tiếng Việt, in đậm khi xuất PDF/In, và unhide hàng/cột/sheet chuẩn Excel).
2. **Copy vùng nhiều ô dạng Ảnh PNG + HTML + Text (`NSPasteboard` Multi-Flavour)**:
   - Khi người dùng copy (`Cmd+C` / `Cmd+X`) một vùng nhiều ô (`range.start != range.end`), `crates/ui-egui` kết xuất vùng ô được chọn (nền ô, đường lưới mảnh, viền ô, ô gộp, căn lề, màu chữ và văn bản tiếng Việt sắc nét ở độ phân giải 2x Retina) thành bộ nhớ đệm ảnh PNG (`Vec<u8>`), giới hạn tối đa 100 dòng × 30 cột và kích thước ảnh tối đa `4096 × 4096` px.
   - Trên macOS (`apps/gridcraft/src/main.rs`), sử dụng `objc2_app_kit::NSPasteboard` và `objc2_foundation::{NSData, NSString}` (an toàn 100%, không dùng `unsafe`) để ghi đồng thời 3 định dạng (`public.png`, `public.html`, `public.utf8-plain-text`) vào cùng một phiên clipboard sau một lệnh `clearContents()` duy nhất.
   - Khi copy 1 ô đơn lẻ (`range.start == range.end`), chỉ ghi `HTML` + `Plain Text` (không ghi PNG) để dán nhanh vào thanh chat hoặc ô tìm kiếm.
   - Đảm bảo dán ngược vào trong GridCraft (`Cmd+V`) vẫn ưu tiên dữ liệu bảng/công thức/text nội bộ thay vì chèn nhầm ảnh nổi.

## 2. Ngữ cảnh Kỹ thuật (Technical Context)

- **Ngôn ngữ & Phiên bản**: Rust Edition 2024 (`rustc 1.85+`, chạy qua `~/.cargo/bin/cargo`).
- **Ràng buộc An toàn**: Workspace cấu hình `unsafe_code = "forbid"` trong `Cargo.toml`. Mọi API macOS (`NSPasteboard`, `NSData`, `NSString`) phải gọi qua các safe wrapper của `objc2-app-kit = "0.3.2"` và `objc2-foundation = "0.3.2"`.
- **Dependencies liên quan**:
  - `crates/ui-egui`: `egui`, `image` (ghi PNG qua `image::codecs::png::PngEncoder` hoặc `RgbaImage`), `gridcraft-model`.
  - `apps/gridcraft`: `arboard` (fallback cho non-macOS), `objc2-app-kit` (bật feature `NSPasteboard`), `objc2-foundation` (bật feature `NSData`, `NSString`).
- **Kiểm thử**: `cargo test` trên `crates/ui-egui` và `crates/engine`, cộng với kiểm tra cú pháp AST (`ast_verify_syntax`) và chẩn đoán kiểu (`lsp_diagnostics`).

## 3. Kiểm tra Hiến pháp Dự án (Constitution Check)

- **P1 — Bảo toàn 100% tính năng tùy chỉnh Việt Nam (`vn-edition`)**: Đạt. Tất cả tùy chỉnh trong `crates/model`, `crates/engine`, `crates/ui-egui` được giữ nguyên khi merge `v0.18.0`.
- **P2 — Tương thích Hệ sinh thái macOS & Clipboard Đa định dạng (Excel/Zalo Parity)**: Đạt. Ghi đồng thời `public.png`, `public.html`, `public.utf8-plain-text` trên `NSPasteboard` khi copy nhiều ô.
- **P3 — Tuân thủ Nghiêm ngặt Kiến trúc Rust của GridCraft**: Đạt. Không sử dụng `unsafe` (`unsafe_code = "forbid"`), tách biệt rõ tầng kết xuất ảnh vùng chọn (`crates/ui-egui`) và tầng dịch vụ hệ thống (`apps/gridcraft`).
- **P4 — Đóng gói & Triển khai Chuẩn xác trên Máy Người dùng**: Đạt. Build `--release`, cài đặt qua `/tmp/GridCraft.app` vào `/Applications/GridCraft.app` + `/opt/homebrew/bin/gridcraft-cli`, ký `codesign --force --deep --sign -` và dọn dẹp `cargo clean`.

## 4. Cấu trúc Dự án & Các File Thay đổi (Project Structure)

```text
/Users/pain/.gemini/antigravity/scratch/gridcraft/
├── apps/gridcraft/
│   ├── Cargo.toml                        # Thêm features NSPasteboard, NSData, NSString cho macOS
│   └── src/main.rs                       # Cập nhật copy_html service ghi đa định dạng (PNG + HTML + Text) lên NSPasteboard
└── crates/ui-egui/
    ├── src/lib.rs                        # Mở rộng chữ ký callback Services::copy_html nhận thêm Option<&[u8]> (PNG bytes)
    ├── src/app/mod.rs                    # Đăng ký module clip_image
    ├── src/app/clipboard.rs              # Cập nhật copy_to_clipboard: tạo ảnh PNG khi copy nhiều ô (range.start != range.end)
    └── src/app/clip_image.rs             # [MỚI] Kết xuất vùng nhiều ô (nền, lưới, viền, ô gộp, chữ tiếng Việt 2x) ra PNG bytes + Unit tests
```

## 5. Thiết kế Chi tiết Kết xuất Ảnh Vùng Copy (`clip_image.rs`)

1. **Điều kiện kích hoạt**:
   - Chỉ tạo ảnh PNG khi `range.start != range.end` (ít nhất 2 ô).
   - Giới hạn an toàn: tối đa `MAX_CLIP_ROWS = 100` dòng và `MAX_CLIP_COLS = 30` cột hiển thị (bỏ qua dòng/cột đang ẩn `hidden_rows` / `hidden_cols`), kích thước pixel tối đa `4096 × 4096` ở tỉ lệ `scale = 2.0` (Retina).
2. **Quy trình vẽ Offscreen (`render_range_to_png`)**:
   - Tính toán tọa độ cột (`x`) và dòng (`y`) của các dòng/cột hiển thị trong `RangeRef`.
   - Khởi tạo buffer `image::RgbaImage` nền trắng (`#FFFFFF`).
   - **Lớp 1 (Nền ô & Đường lưới)**: Tô màu nền (`style.fill.bg_color`) cho từng ô/vùng gộp và vẽ đường lưới mảnh (`#D4D4D4`) như bảng tính Excel.
   - **Lớp 2 (Viền ô — Borders)**: Vẽ viền `top`, `bottom`, `left`, `right` theo kiểu dáng và màu sắc của `CellStyle::borders`.
   - **Lớp 3 (Văn bản & Căn lề)**: Sử dụng font atlas hiện có của `egui::Context` (`ctx.fonts_mut` + `font_image_delta` / UV texture của `egui`) để lấy chính xác vị trí từng glyph đã được định hình (shaping/fallback tiếng Việt, in đậm, in nghiêng, cỡ chữ, màu chữ, căn trái/giữa/phải) và pha trộn (alpha-blend) trực tiếp vào `RgbaImage`.
   - Mã hóa `RgbaImage` thành bộ nhớ đệm PNG (`Vec<u8>`) bằng `image::codecs::png::PngEncoder`.

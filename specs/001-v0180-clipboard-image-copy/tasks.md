# Danh sách Công việc (Tasks): Nâng cấp GridCraft v0.18.0 & Copy nhiều ô dạng Ảnh + Text

**Tính năng**: `001-v0180-clipboard-image-copy`
**Spec**: [spec.md](file:///Users/pain/.gemini/antigravity/scratch/gridcraft/specs/001-v0180-clipboard-image-copy/spec.md) | **Plan**: [plan.md](file:///Users/pain/.gemini/antigravity/scratch/gridcraft/specs/001-v0180-clipboard-image-copy/plan.md)

## Phase 1: Setup & Đồng bộ Upstream v0.18.0 (`US2`)

- [ ] T001 [US2] Kiểm tra trạng thái git merge `v0.18.0` và các bài test sửa đổi tương thích (`crates/model/src/sheet.rs`, `crates/ui-egui/tests/it/font_size_box.rs`, `crates/ui-egui/tests/it/form_controls.rs`) để đảm bảo bảo toàn 100% tính năng tùy chỉnh bản Việt Nam (`FR-001`).

## Phase 2: Foundational — Mở rộng Giao diện Clipboard Service (`US1`, `US3`)

- [ ] T002 Cập nhật kiểu callback `CopyHtmlFn` trong `crates/ui-egui/src/lib.rs` từ `Fn(&str, &str)` thành `Fn(&str, &str, Option<&[u8]>)` để truyền dữ liệu ảnh PNG tùy chọn cùng với HTML và Plain Text (`FR-002`, `FR-004`).

## Phase 3: User Story 1 & 3 — Kết xuất Ảnh Vùng Copy & Ghi Đa định dạng lên macOS Pasteboard (`US1` P1, `US3` P1)

- [ ] T003 [US1] Tạo module `crates/ui-egui/src/app/clip_image.rs` (và khai báo trong `crates/ui-egui/src/app/mod.rs`) cài đặt hàm `render_range_to_png(&self, ctx: &egui::Context, range: RangeRef) -> Option<Vec<u8>>` kết xuất nền ô, đường lưới, viền ô, ô gộp và văn bản tiếng Việt ở tỉ lệ 2x Retina cho vùng nhiều ô (`range.start != range.end`), giới hạn tối đa 100 dòng × 30 cột và `4096 × 4096` px (`FR-003`, `FR-004`).
- [ ] T004 [US1] [US3] Cập nhật `SheetApp::copy_to_clipboard` trong `crates/ui-egui/src/app/clipboard.rs` để phân tích vùng `"range"` trả về từ lệnh `copy`/`cut`, gọi `self.render_range_to_png(ctx, range)` khi vùng chọn có nhiều hơn 1 ô (`range.start != range.end`), và truyền `None` khi chỉ copy 1 ô đơn lẻ (`FR-002`, `FR-004`, `FR-005`).
- [ ] T005 [P] [US1] Cập nhật `apps/gridcraft/Cargo.toml` bổ sung features `NSPasteboard` cho `objc2-app-kit` và `NSData`, `NSString` cho `objc2-foundation` trên `target_os = "macos"` (`FR-002`).
- [ ] T006 [US1] [US3] Cài đặt hàm ghi clipboard đa định dạng trên macOS trong `apps/gridcraft/src/main.rs` sử dụng `NSPasteboard::generalPasteboard()` (không dùng `unsafe`) để ghi đồng thời `public.png` (khi có PNG), `public.html`, và `public.utf8-plain-text` trong cùng một lần `clearContents()`, đảm bảo Zalo nhận được cả ảnh lẫn tùy chọn "Dán dạng văn bản" và GridCraft `Cmd+V` vẫn dán dữ liệu ô bình thường (`FR-002`, `FR-005`).

## Phase 4: Polish, Kiểm thử & Đóng gói Cài đặt (`US1`, `US2`, `US3`)

- [ ] T007 Viết unit tests trong `crates/ui-egui/src/app/clip_image.rs` kiểm tra: (1) copy 1 ô đơn lẻ trả về `None` (không tạo PNG), (2) copy vùng nhiều ô có chữ tiếng Việt và định dạng trả về dữ liệu PNG hợp lệ có header `\x89PNG`, và chạy `ast_verify_syntax` + `cargo test` (`SC-002`, `SC-003`, `SC-004`).
- [ ] T008 Build bản phát hành tối ưu (`~/.cargo/bin/cargo build --release -p gridcraft -p gridcraft-cli`), đóng gói vào `/Applications/GridCraft.app` (kèm phiên bản `0.18.0` trong `Info.plist`) và `/opt/homebrew/bin/gridcraft-cli`, ký `codesign --force --deep --sign -` (`SC-001`).
- [ ] T009 Commit hoàn tất merge `v0.18.0` + tính năng copy ảnh/text lên nhánh `vn-edition`, đẩy lên remote `origin` (`dinhsonconstruction/gridcraft-vn`), và chạy `~/.cargo/bin/cargo clean` để giải phóng dung lượng đĩa.

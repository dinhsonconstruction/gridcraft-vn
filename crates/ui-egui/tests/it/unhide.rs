//! End-to-end UI tests for all 4 Unhide fixes:
//! 1. Header right-click range promotion (`A33:C51` -> `33:51` / `B2:F10` -> `B:F`) without full-sheet explosion.
//! 2. Cell context menu quick-access `Unhide Rows` / `Unhide Columns` (English + Vietnamese).
//! 3. Header double-click on both sides of a hidden seam unhiding the entire contiguous span instead of AutoFitting.
//! 4. Excel-style searchable, multi-selectable `Unhide Sheet` dialog excluding `VeryHidden` sheets.
//! 5. macOS `Ctrl+(Shift)+9/0` and `Cmd+(Shift)+9/0` keyboard shortcuts without conflicting with `Cmd+Option+0`.

use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, os::OperatingSystem, pos2, vec2};
use egui_kittest::kittest::Queryable as _;
use gridcraft_engine::{Session, model::Visibility};
use gridcraft_ui_egui::{
    SheetApp,
    grid::Geo,
    i18n::Language,
    keymap::{Shortcut, lookup},
};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness(session: Session) -> Harness {
    let mut h = crate::common::builder()
        .with_os(OperatingSystem::Mac)
        .with_size(vec2(1200.0, 800.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, app: &mut SheetApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            crate::common::app(session),
        );
    h.run_steps(4);
    h
}

fn right_click(h: &mut Harness, pos: Pos2) {
    h.input_mut().events.extend([
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE },
    ]);
    h.step();
    h.input_mut()
        .events
        .push(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

fn primary_button(h: &mut Harness, pos: Pos2, pressed: bool) {
    h.input_mut().events.extend([
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
    ]);
    h.step();
}

fn double_click(h: &mut Harness, pos: Pos2) {
    // Ensure prior double-click windows (> 0.3s) have elapsed so back-to-back calls within 6px
    // are not coalesced into a triple-click by egui.
    h.run_steps(25);
    primary_button(h, pos, true);
    primary_button(h, pos, false);
    h.step();
    primary_button(h, pos, true);
    primary_button(h, pos, false);
    h.run_steps(3);
}

#[test]
fn right_clicking_header_within_multi_cell_selection_promotes_to_full_lines_and_unhides() {
    // 1. Row header right-click while A5:C18 is selected promotes to full rows 5:18 and unhides 6:17.
    let mut s = Session::new();
    s.new_workbook();
    s.execute("home.hideRows", json!({"rows": "6:17"})).unwrap();
    s.execute("selection.set", json!({"range": "A5:C18"})).unwrap();
    let mut h = harness(s);

    let row5_header = {
        let app = h.state();
        let sh = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
        pos2(geo.rect.left() + geo.header_w * 0.5, (geo.y(sh, 4) + geo.y(sh, 5)) * 0.5)
    };
    right_click(&mut h, row5_header);
    assert_eq!(
        h.state().session.active().unwrap().selection.ranges[0].a1(),
        "5:18",
        "right-clicking row header inside A5:C18 must promote to full rows 5:18"
    );
    h.get_by_label("Unhide").click();
    h.run_steps(3);
    for r in 5..=16 {
        assert!(!h.state().session.active().unwrap().wb.active().unwrap().is_row_hidden(r), "row {} unhidden", r + 1);
    }

    // 2. Full-column selection guard: when A:C is selected, right-clicking row 5 must NOT select 1:1048576.
    h.state_mut().run("selection.set", json!({"range": "A:C"})).unwrap();
    h.run_steps(2);
    right_click(&mut h, row5_header);
    assert_eq!(
        h.state().session.active().unwrap().selection.ranges[0].a1(),
        "5:5",
        "full-column selection A:C must not explode to whole sheet on row header right-click"
    );

    // 3. Column header symmetry: B2:F10 promoted to B:F on column B header right-click.
    let mut s2 = Session::new();
    s2.new_workbook();
    s2.execute("home.hideColumns", json!({"cols": "C:E"})).unwrap();
    s2.execute("selection.set", json!({"range": "B2:F10"})).unwrap();
    let mut h2 = harness(s2);
    let col_b_header = {
        let app = h2.state();
        let sh = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
        pos2((geo.x(sh, 1) + geo.x(sh, 2)) * 0.5, geo.rect.top() + geo.header_h * 0.5)
    };
    right_click(&mut h2, col_b_header);
    assert_eq!(h2.state().session.active().unwrap().selection.ranges[0].a1(), "B:F");
    h2.get_by_label("Unhide").click();
    h2.run_steps(3);
    for c in 2..=4 {
        assert!(!h2.state().session.active().unwrap().wb.active().unwrap().is_col_hidden(c), "col {c} unhidden");
    }
}

#[test]
fn cell_context_menu_offers_quick_unhide_rows_and_columns_when_adjacent_or_spanning() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("home.hideRows", json!({"rows": "3:5"})).unwrap();
    s.execute("home.hideColumns", json!({"cols": "C:D"})).unwrap();
    s.execute("selection.set", json!({"cell": "B2"})).unwrap();
    let mut h = harness(s);

    // Cell B2 sits immediately adjacent to hidden row 3 and hidden column C -> both quick actions appear.
    let b2_center = {
        let app = h.state();
        let sh = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
        geo.cell_rect(sh, gridcraft_engine::core::CellRef::new(1, 1)).center()
    };
    right_click(&mut h, b2_center);
    assert!(h.query_by_label("Unhide Rows").is_some(), "Unhide Rows shown when adjacent to hidden rows");
    assert!(h.query_by_label("Unhide Columns").is_some(), "Unhide Columns shown when adjacent to hidden cols");
    h.get_by_label("Unhide Rows").click();
    h.run_steps(3);
    for r in 2..=4 {
        assert!(!h.state().session.active().unwrap().wb.active().unwrap().is_row_hidden(r));
    }

    // Far away cell H10 has no adjacent hidden rows/cols -> menu stays clean.
    h.state_mut().run("selection.set", json!({"cell": "H10"})).unwrap();
    h.run_steps(2);
    let h10_center = {
        let app = h.state();
        let sh = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
        geo.cell_rect(sh, gridcraft_engine::core::CellRef::new(9, 7)).center()
    };
    right_click(&mut h, h10_center);
    assert!(h.query_by_label("Unhide Rows").is_none());
    assert!(h.query_by_label("Unhide Columns").is_none());

    // Vietnamese translation verification in i18n catalog.
    assert_eq!(Language::Vi.tr("Unhide Rows"), "Bỏ ẩn hàng");
    assert_eq!(Language::Vi.tr("Unhide Columns"), "Bỏ ẩn cột");
    assert_eq!(Language::Vi.tr("Unhide All"), "Bỏ ẩn tất cả");
}

#[test]
fn double_clicking_either_side_of_hidden_header_seam_unhides_block_instead_of_autofitting() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "Very long text that AutoFit would change"})).unwrap();
    s.execute("home.columnWidth", json!({"cols": "A:E", "width": 80})).unwrap();
    s.execute("home.rowHeight", json!({"rows": "1:6", "height": 40})).unwrap();
    s.execute("home.hideRows", json!({"rows": "2:4"})).unwrap();
    s.execute("home.hideColumns", json!({"cols": "B:D"})).unwrap();
    let mut h = harness(s);
    h.state_mut().session.journal.clear();
    h.state_mut().session.active_mut().unwrap().undo.clear();

    // Seam between visible row 1 (idx 0) and hidden rows 2:4 (idx 1..=3): test -2px BEFORE and +2px AFTER seam.
    for dy in [-2.0_f32, 2.0_f32] {
        let seam_pos = {
            let app = h.state();
            let sh = app.session.active().unwrap().wb.active().unwrap();
            let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
            pos2(geo.rect.left() + geo.header_w * 0.5, geo.y(sh, 1) + dy)
        };
        double_click(&mut h, seam_pos);
        {
            let sh = h.state().session.active().unwrap().wb.active().unwrap();
            for r in 1..=3 {
                assert!(!sh.is_row_hidden(r), "dy={dy}: row {} must be unhidden", r + 1);
            }
            assert_eq!(sh.row_height(0), 40.0, "dy={dy}: row 1 must NOT be AutoFitted");
        }
        assert!(
            !h.state().session.journal.iter().any(|(id, _)| id == "home.autofitRowHeight"),
            "dy={dy}: must not misfire home.autofitRowHeight"
        );
        h.state_mut().session.execute("edit.undo", json!({})).unwrap();
        h.run_steps(2);
    }

    // Seam between visible col A (idx 0) and hidden cols B:D (idx 1..=3): test -2px BEFORE and +2px AFTER seam.
    for dx in [-2.0_f32, 2.0_f32] {
        let seam_pos = {
            let app = h.state();
            let sh = app.session.active().unwrap().wb.active().unwrap();
            let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
            pos2(geo.x(sh, 1) + dx, geo.rect.top() + geo.header_h * 0.5)
        };
        double_click(&mut h, seam_pos);
        {
            let sh = h.state().session.active().unwrap().wb.active().unwrap();
            for c in 1..=3 {
                assert!(!sh.is_col_hidden(c), "dx={dx}: col {c} must be unhidden");
            }
            assert_eq!(sh.col_width(0), 80.0, "dx={dx}: col A must NOT be AutoFitted");
        }
        assert!(
            !h.state().session.journal.iter().any(|(id, _)| id == "home.autofitColumnWidth"),
            "dx={dx}: must not misfire home.autofitColumnWidth"
        );
        h.state_mut().session.execute("edit.undo", json!({})).unwrap();
        h.run_steps(2);
    }
}

#[test]
fn unhide_sheet_dialog_excludes_very_hidden_preserves_trailing_space_and_supports_unhide_all() {
    let mut s = Session::new();
    s.new_workbook();
    for name in ["CPXD ", "GiaiDoan2", "TongHop", "SGV", "foxz"] {
        s.execute("home.insertSheet", json!({"name": name})).unwrap();
    }
    for name in ["CPXD ", "GiaiDoan2", "TongHop"] {
        s.execute("sheet.hide", json!({"sheet": name})).unwrap();
    }
    {
        let wb = Arc::make_mut(&mut s.active_mut().unwrap().wb);
        Arc::make_mut(&mut wb.sheets[4]).visibility = Visibility::VeryHidden;
        Arc::make_mut(&mut wb.sheets[5]).visibility = Visibility::VeryHidden;
    }
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    let mut h = harness(s);

    // Open Unhide Sheet dialog through the real command (`sheet.unhide` with `{}`).
    h.state_mut().run_or_alert("sheet.unhide", json!({}));
    h.run_steps(4);
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name.as_str()), Some("unhideSheet"));
    // VeryHidden sheets SGV and foxz must never appear in the UI!
    assert!(h.query_by_label("SGV").is_none(), "VeryHidden SGV must not appear in Unhide Sheet dialog");
    assert!(h.query_by_label("foxz").is_none(), "VeryHidden foxz must not appear in Unhide Sheet dialog");
    assert!(h.query_by_label("CPXD ").is_some());
    assert!(h.query_by_label("GiaiDoan2").is_some());

    // Confirming default selection unhides "CPXD " cleanly despite its trailing space!
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(h.state().dialog.is_none());
    assert_eq!(h.state().session.active().unwrap().wb.sheets[1].visibility, Visibility::Visible);

    // Re-open Unhide Sheet and click "Unhide All" -> unhides both GiaiDoan2 and TongHop at once, leaving SGV/foxz VeryHidden.
    h.state_mut().run_or_alert("sheet.unhide", json!({}));
    h.run_steps(4);
    h.get_by_label("Unhide All").click();
    h.run_steps(3);
    assert!(h.state().dialog.is_none());
    {
        let wb = &h.state().session.active().unwrap().wb;
        assert_eq!(wb.sheets[2].visibility, Visibility::Visible);
        assert_eq!(wb.sheets[3].visibility, Visibility::Visible);
        assert_eq!(wb.sheets[4].visibility, Visibility::VeryHidden);
        assert_eq!(wb.sheets[5].visibility, Visibility::VeryHidden);
    }
}

#[test]
fn macos_hide_and_unhide_shortcuts_support_both_ctrl_and_cmd_without_conflicts() {
    let mac_ctrl = Modifiers::CTRL;
    let mac_ctrl_shift = Modifiers { shift: true, ..Modifiers::CTRL };
    let mac_cmd = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };
    let mac_cmd_shift = Modifiers { shift: true, ..mac_cmd };
    let mac_cmd_opt = Modifiers { alt: true, ..mac_cmd };
    let run = |id: &'static str| Some(Shortcut::Run(id, json!({})));

    // Lookup table checks on macOS (both logical digit and physical Num9/Num0 when Shift produces punctuation).
    assert_eq!(lookup(Key::Num9, Some(Key::Num9), mac_ctrl, true), run("home.hideRows"));
    assert_eq!(lookup(Key::Num9, Some(Key::Num9), mac_ctrl_shift, true), run("home.unhideRows"));
    assert_eq!(lookup(Key::Num0, Some(Key::Num0), mac_ctrl, true), run("home.hideColumns"));
    assert_eq!(lookup(Key::Num0, Some(Key::Num0), mac_ctrl_shift, true), run("home.unhideColumns"));

    assert_eq!(lookup(Key::Num9, Some(Key::Num9), mac_cmd, true), run("home.hideRows"));
    assert_eq!(lookup(Key::Num9, Some(Key::Num9), mac_cmd_shift, true), run("home.unhideRows"));
    assert_eq!(lookup(Key::Num0, Some(Key::Num0), mac_cmd, true), run("home.hideColumns"));
    assert_eq!(lookup(Key::Num0, Some(Key::Num0), mac_cmd_shift, true), run("home.unhideColumns"));

    // Cmd+Option+0 on Mac remains Outside Borders (no collision).
    assert_eq!(
        lookup(Key::Num0, Some(Key::Num0), mac_cmd_opt, true),
        Some(Shortcut::Run("home.borders", json!({"preset": "outside"})))
    );

    // End-to-end real key events on macOS harness: Ctrl+9 hides row 2, Ctrl+Shift+9 unhides it!
    let mut s = Session::new();
    s.new_workbook();
    s.execute("selection.set", json!({"cell": "B2"})).unwrap();
    let mut h = harness(s);

    h.input_mut().events.push(Event::Key {
        key: Key::Num9,
        physical_key: Some(Key::Num9),
        pressed: true,
        repeat: false,
        modifiers: mac_ctrl,
    });
    h.run_steps(2);
    assert!(h.state().session.active().unwrap().wb.active().unwrap().is_row_hidden(1), "Ctrl+9 hides row 2 on Mac");

    // Select adjacent row 3 and press Ctrl+Shift+9 -> unhides row 2!
    h.state_mut().run("selection.set", json!({"cell": "B3"})).unwrap();
    h.run_steps(2);
    h.input_mut().events.push(Event::Key {
        key: Key::Num9,
        physical_key: Some(Key::Num9),
        pressed: true,
        repeat: false,
        modifiers: mac_ctrl_shift,
    });
    h.run_steps(2);
    assert!(
        !h.state().session.active().unwrap().wb.active().unwrap().is_row_hidden(1),
        "Ctrl+Shift+9 unhides adjacent row 2 on Mac"
    );
}

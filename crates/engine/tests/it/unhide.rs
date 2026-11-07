//! Comprehensive Unhide regression tests (rows, columns, sliver sizes, and sheets).

use std::sync::Arc;

use gridcraft_engine::{
    Session,
    model::{DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, Visibility},
};
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s
}

#[test]
fn single_line_unhide_opens_entire_contiguous_hidden_block_of_twelve_rows() {
    let mut s = session();
    // Hide 12 contiguous rows 34:45 (0-indexed 33..=44, matching Form 02.a/TT) and a separate block 49:50.
    s.execute("home.hideRows", json!({"rows": "34:45"})).unwrap();
    s.execute("home.hideRows", json!({"rows": "49:50"})).unwrap();
    s.active_mut().unwrap().undo.clear();

    // Selecting row 33 (immediately above the 12-row hidden block) unhides all 12 rows 34:45 at once.
    s.execute("selection.set", json!({"range": "33:33"})).unwrap();
    s.execute("home.unhideRows", json!({})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        for r in 33..=44 {
            assert!(!sh.is_row_hidden(r), "row {} (1-based {}) must be unhidden", r, r + 1);
        }
        // Separate hidden block 49:50 (0-indexed 48..=49) remains untouched.
        assert!(sh.is_row_hidden(48));
        assert!(sh.is_row_hidden(49));
    }
    assert_eq!(s.active().unwrap().undo.len(), 1);

    // Undo re-hides 34:45; selecting row 46 (immediately below the 12-row hidden block) also unhides all 12 rows.
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.active().unwrap().wb.active().unwrap().is_row_hidden(33));
    assert!(s.active().unwrap().wb.active().unwrap().is_row_hidden(44));

    s.execute("selection.set", json!({"cell": "B46"})).unwrap();
    s.execute("home.unhideRows", json!({})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        for r in 33..=44 {
            assert!(!sh.is_row_hidden(r), "row {} (1-based {}) must be unhidden from below", r, r + 1);
        }
        assert!(sh.is_row_hidden(48));
        assert!(sh.is_row_hidden(49));
    }
}

#[test]
fn unhide_reaches_row_one_and_column_a_from_adjacent_visible_line() {
    let mut s = session();
    // Hide Row 1..3 (indices 0..=2) and Col A..B (indices 0..=1).
    s.execute("home.hideRows", json!({"rows": "1:3"})).unwrap();
    s.execute("home.hideColumns", json!({"cols": "A:B"})).unwrap();

    // Selecting visible Row 4 unhides Row 1..3 (reaching index 0).
    s.execute("selection.set", json!({"range": "4:4"})).unwrap();
    s.execute("home.unhideRows", json!({})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        for r in 0..=2 {
            assert!(!sh.is_row_hidden(r), "hidden leading row {r} should be unhidden");
        }
    }

    // Selecting visible Column C unhides Column A..B (reaching index 0).
    s.execute("selection.set", json!({"range": "C:C"})).unwrap();
    s.execute("home.unhideColumns", json!({})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        for c in 0..=1 {
            assert!(!sh.is_col_hidden(c), "hidden leading col {c} should be unhidden");
        }
    }
}

#[test]
fn multi_line_selection_landing_mid_block_expands_to_full_hidden_run() {
    let mut s = session();
    s.execute("home.hideRows", json!({"rows": "10:15"})).unwrap();
    s.execute("home.hideRows", json!({"rows": "20:22"})).unwrap();

    // Endpoint 12 lands inside the hidden block 10:15 -> expands cleanly through 15 without touching 20:22.
    s.execute("home.unhideRows", json!({"rows": "8:12"})).unwrap();
    let sh = s.active().unwrap().wb.active().unwrap();
    for r in 9..=14 {
        assert!(!sh.is_row_hidden(r), "row {} should be unhidden", r + 1);
    }
    for r in 19..=21 {
        assert!(sh.is_row_hidden(r), "row {} in separate block should stay hidden", r + 1);
    }
}

#[test]
fn unhide_resets_sliver_row_heights_and_column_widths_while_preserving_normal_custom_sizes() {
    let mut s = session();
    // Sliver columns: C has chars=1.0 (like Excel XML width="1"), D has width=0.0, E has legitimate narrow chars=4.0.
    s.execute("home.columnWidth", json!({"cols": "C:C", "chars": 1.0})).unwrap();
    s.execute("home.columnWidth", json!({"cols": "D:D", "width": 0.0})).unwrap();
    s.execute("home.columnWidth", json!({"cols": "E:E", "chars": 4.0})).unwrap();
    let normal_col_px = s.active().unwrap().wb.active().unwrap().col_width(4);
    assert!(normal_col_px > 20.0 && normal_col_px < DEFAULT_COL_WIDTH);

    s.execute("home.hideColumns", json!({"cols": "C:E"})).unwrap();
    s.active_mut().unwrap().undo.clear();
    s.execute("home.unhideColumns", json!({"cols": "B:F"})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        assert_eq!(sh.col_width(2), DEFAULT_COL_WIDTH, "sliver chars=1.0 col C resets to default width");
        assert_eq!(sh.col_width(3), DEFAULT_COL_WIDTH, "zero-width col D resets to default width");
        assert_eq!(sh.col_width(4), normal_col_px, "legitimate narrow col E keeps its custom width");
    }

    // Undo restores hidden state and original sliver widths.
    s.execute("edit.undo", json!({})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        assert!(sh.is_col_hidden(2));
        assert!(sh.cols.get(&2).and_then(|e| e.size).is_some_and(|w| w < 16.0));
    }

    // Sliver rows: row 5 has points=1.5 (<= 2.0pt), row 6 has height=0.0, row 7 has custom points=30.0 (40px).
    s.execute("home.rowHeight", json!({"rows": "5:5", "points": 1.5})).unwrap();
    s.execute("home.rowHeight", json!({"rows": "6:6", "height": 0.0})).unwrap();
    s.execute("home.rowHeight", json!({"rows": "7:7", "points": 30.0})).unwrap();
    s.execute("home.hideRows", json!({"rows": "5:7"})).unwrap();
    s.execute("home.unhideRows", json!({"rows": "4:8"})).unwrap();
    {
        let sh = s.active().unwrap().wb.active().unwrap();
        assert_eq!(sh.row_height(4), DEFAULT_ROW_HEIGHT, "sliver 1.5pt row 5 resets to default height");
        assert!(!sh.rows.get(&4).is_some_and(|e| e.custom), "reset row clears custom flag");
        assert_eq!(sh.row_height(5), DEFAULT_ROW_HEIGHT, "zero-height row 6 resets to default height");
        assert!((sh.row_height(6) - 40.0).abs() < 0.5, "custom 30pt row 7 keeps 40px height");
    }
}

#[test]
fn sheet_unhide_excludes_very_hidden_and_unhides_multiple_sheets_with_whitespace_atomically() {
    let mut s = session();
    s.execute("sheet.rename", json!({"sheet": 0, "name": "Bìa"})).unwrap();
    for name in ["CPXD ", " Thong ke TN 3.1", "SGV", "foxz"] {
        s.execute("home.insertSheet", json!({"name": name})).unwrap();
    }
    s.execute("sheet.hide", json!({"sheet": "CPXD "})).unwrap();
    s.execute("sheet.hide", json!({"sheet": " Thong ke TN 3.1"})).unwrap();
    {
        let wb = Arc::make_mut(&mut s.active_mut().unwrap().wb);
        Arc::make_mut(&mut wb.sheets[3]).visibility = Visibility::VeryHidden;
        Arc::make_mut(&mut wb.sheets[4]).visibility = Visibility::VeryHidden;
    }
    s.active_mut().unwrap().undo.clear();

    // Calling sheet.unhide without params queues the unhideSheet UiRequest with ONLY Visibility::Hidden sheets (never VeryHidden).
    s.ui_requests.clear();
    s.execute("sheet.unhide", json!({})).unwrap();
    assert_eq!(s.ui_requests.len(), 1);
    match &s.ui_requests[0] {
        gridcraft_engine::UiRequest::Dialog(name, payload) => {
            assert_eq!(name, "unhideSheet");
            assert_eq!(payload["sheets"], json!(["CPXD ", " Thong ke TN 3.1"]));
        }
        other => panic!("expected Dialog UiRequest, got {other:?}"),
    }

    // Atomic multi-sheet unhide preserves exact leading/trailing spaces in sheet names and records 1 undo step.
    let res = s.execute("sheet.unhide", json!({"sheets": ["CPXD ", " Thong ke TN 3.1"]})).unwrap();
    assert_eq!(res["count"], 2);
    assert_eq!(s.active().unwrap().undo.len(), 1);
    {
        let wb = &s.active().unwrap().wb;
        assert_eq!(wb.sheets[1].visibility, Visibility::Visible);
        assert_eq!(wb.sheets[2].visibility, Visibility::Visible);
        assert_eq!(wb.sheets[3].visibility, Visibility::VeryHidden);
        assert_eq!(wb.sheets[4].visibility, Visibility::VeryHidden);
    }

    // With only VeryHidden sheets left, sheet.unhide {} reports "No sheets are hidden." instead of leaking SGV/foxz.
    let err = s.execute("sheet.unhide", json!({})).unwrap_err();
    assert!(err.to_string().contains("No sheets are hidden"), "unexpected error: {err}");

    // Single undo re-hides both sheets atomically.
    s.execute("edit.undo", json!({})).unwrap();
    let wb = &s.active().unwrap().wb;
    assert_eq!(wb.sheets[1].visibility, Visibility::Hidden);
    assert_eq!(wb.sheets[2].visibility, Visibility::Hidden);
}

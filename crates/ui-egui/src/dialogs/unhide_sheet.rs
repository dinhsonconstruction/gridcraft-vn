use super::{Dialog, Json, Key, SheetApp, json, vec2};

/// Excel-style Unhide Sheet dialog with a searchable, multi-selectable sheet picker.
/// Only `Visibility::Hidden` sheets are shown (`VeryHidden` system sheets are never listed),
/// and exact sheet names (including leading/trailing spaces) are preserved verbatim.
pub(super) fn unhide_sheet(app: &SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let lang = app.ui.language;
    let options: Vec<String> = d
        .values
        .get("options")
        .and_then(Json::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let mut selected: Vec<String> = d
        .values
        .get("selected")
        .and_then(Json::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_else(|| {
            d.values
                .get("sheet")
                .and_then(Json::as_str)
                .filter(|s| !s.is_empty())
                .map(|s| vec![s.to_string()])
                .unwrap_or_default()
        });

    let (down, up, home, end, pgup, pgdn) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            i.consume_key(egui::Modifiers::CTRL, Key::Home),
            i.consume_key(egui::Modifiers::CTRL, Key::End),
            i.consume_key(egui::Modifiers::NONE, Key::PageUp),
            i.consume_key(egui::Modifiers::NONE, Key::PageDown),
        )
    });

    ui.label(lang.tr("Unhide sheet:"));
    let width = ui.available_width();
    let search_resp = ui.add(
        egui::TextEdit::singleline(&mut d.search)
            .desired_width(width)
            .hint_text(lang.tr("Filter sheets…")),
    );
    if !d.focus_initialized {
        search_resp.request_focus();
        d.focus_initialized = true;
    }

    let query = d.search.trim().to_lowercase();
    let filtered: Vec<&String> = options
        .iter()
        .filter(|name| query.is_empty() || name.to_lowercase().contains(&query))
        .collect();

    if search_resp.changed() {
        d.list_index = 0;
        if let Some(first) = filtered.first() {
            if !selected.iter().any(|s| filtered.iter().any(|f| *f == s)) {
                selected = vec![(*first).clone()];
            }
        } else {
            selected.clear();
        }
    }

    const LIST_HEIGHT: f32 = 220.0;
    let row_h = 22.0_f32;
    let page_size = ((LIST_HEIGHT / (row_h + 2.0)).floor() as usize).saturating_sub(1).max(1);

    if !filtered.is_empty() {
        let last = filtered.len() - 1;
        d.list_index = d.list_index.min(last);
        let nav = down || up || home || end || pgup || pgdn;
        if down {
            d.list_index = (d.list_index + 1).min(last);
        }
        if up {
            d.list_index = d.list_index.saturating_sub(1);
        }
        if pgdn {
            d.list_index = (d.list_index + page_size).min(last);
        }
        if pgup {
            d.list_index = d.list_index.saturating_sub(page_size);
        }
        if home {
            d.list_index = 0;
        }
        if end {
            d.list_index = last;
        }
        if nav {
            selected = vec![filtered[d.list_index].clone()];
        }
    }

    ui.add_space(4.0);
    let mods = ui.input(|i| i.modifiers);
    egui::Frame::canvas(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
        ui.set_min_width((width - 8.0).max(100.0));
        egui::ScrollArea::vertical()
            .id_salt("unhide_sheet_list")
            .max_height(LIST_HEIGHT)
            .min_scrolled_height(120.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if filtered.is_empty() {
                    ui.weak(lang.tr("No matching hidden sheets."));
                    return;
                }
                let row_w = ui.available_width();
                let nav = down || up || home || end || pgup || pgdn;
                for (i, &name) in filtered.iter().enumerate() {
                    let is_sel = selected.iter().any(|s| s == name);
                    let r = ui.add(egui::Button::selectable(is_sel, name.as_str()).truncate().min_size(vec2(row_w, row_h)));
                    if nav && i == d.list_index && !ui.clip_rect().contains_rect(r.rect) {
                        r.scroll_to_me(Some(if down || end || pgdn { egui::Align::Max } else { egui::Align::Min }));
                    }
                    if name.trim() != name {
                        r.clone().on_hover_text(format!("{name:?}"));
                    }
                    if r.double_clicked() {
                        d.list_index = i;
                        selected = vec![name.clone()];
                        *confirm = true;
                    } else if r.clicked() {
                        if mods.shift && !filtered.is_empty() {
                            let lo = d.list_index.min(i);
                            let hi = d.list_index.max(i);
                            selected = filtered[lo..=hi].iter().map(|s| (*s).clone()).collect();
                        } else if mods.command || mods.ctrl {
                            if let Some(pos) = selected.iter().position(|s| s == name) {
                                if selected.len() > 1 {
                                    selected.remove(pos);
                                }
                            } else {
                                selected.push(name.clone());
                            }
                            d.list_index = i;
                        } else {
                            d.list_index = i;
                            selected = vec![name.clone()];
                        }
                    }
                }
            });
    });

    // Keep only valid hidden sheet names in original workbook order.
    let valid_selected: Vec<String> = options.iter().filter(|opt| selected.iter().any(|s| s == *opt)).cloned().collect();
    selected = valid_selected;

    ui.add_space(8.0);
    let can_confirm = !selected.is_empty();
    let mut unhide_all_clicked = false;
    ui.horizontal(|ui| {
        if filtered.len() > 1 && ui.add(egui::Button::new(lang.tr("Unhide All")).min_size(egui::vec2(84.0, 0.0))).clicked() {
            unhide_all_clicked = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let t = crate::theme::Tokens::get(ui.ctx());
            let ok_btn = egui::Button::new(egui::RichText::new(lang.tr("OK")).color(t.on_accent))
                .fill(t.accent)
                .min_size(egui::vec2(72.0, 0.0));
            let ok = ui.add_enabled(can_confirm, ok_btn);
            if can_confirm && (ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter))) {
                *confirm = true;
            }
            if ui.add(egui::Button::new(lang.tr("Cancel")).min_size(egui::vec2(72.0, 0.0))).clicked() {
                *open = false;
            }
        });
    });

    if unhide_all_clicked {
        selected = filtered.iter().map(|s| (*s).clone()).collect();
        *confirm = true;
    }

    if let Some(first) = selected.first() {
        d.values.insert("sheet".into(), json!(first));
    } else {
        d.values.remove("sheet");
    }
    d.values.insert("selected".into(), json!(selected));
}

use eframe::egui;
use egui::{Color32, RichText};

use egui_extras::{Column, TableBuilder};

use pantones_core::model::{Pantone, format_weight};
use pantones_core::storage::{Storage, load_recent_paths, save_recent_paths};

const BG: Color32 = Color32::from_rgb(19, 22, 30);
const PANEL: Color32 = Color32::from_rgb(26, 30, 41);
const PANEL_HI: Color32 = Color32::from_rgb(34, 39, 53);
const ACCENT: Color32 = Color32::from_rgb(130, 178, 190);
const TEXT: Color32 = Color32::from_rgb(196, 202, 216);
const GRAY: Color32 = Color32::from_rgb(158, 165, 181);
const FAINT: Color32 = Color32::from_rgb(84, 92, 112);
const SELECT_BG: Color32 = Color32::from_rgb(52, 62, 92);
const ERROR: Color32 = Color32::from_rgb(232, 110, 110);

#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    None,
    NumberAsc,
    NumberDesc,
    WeightAsc,
    WeightDesc,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Modal {
    None,
    Add,
    Edit(usize),
    Delete(usize),
}

pub struct GuiApp {
    file: String,
    pantones: Vec<Pantone>,
    visible: Vec<usize>,
    filter: String,
    selected: Option<usize>,
    message: Option<String>,

    modal: Modal,
    form_number: String,
    form_r: String,
    form_g: String,
    form_b: String,
    form_weight: String,
    form_error: Option<String>,
    sort_mode: SortMode,
    focus_form_first: bool,
    focus_search: bool,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file: String) -> Self {
        cc.egui_ctx.set_visuals(theme());
        let (pantones, file, message) = Self::load(file);
        Self {
            file,
            visible: (0..pantones.len()).collect(),
            pantones,
            filter: String::new(),
            selected: None,
            message,
            modal: Modal::None,
            form_number: String::new(),
            form_r: String::new(),
            form_g: String::new(),
            form_b: String::new(),
            form_weight: String::new(),
            form_error: None,
            sort_mode: SortMode::None,
            focus_form_first: false,
            focus_search: false,
        }
    }

    fn load(file: String) -> (Vec<Pantone>, String, Option<String>) {
        let storage = Storage::new(&file);
        let exists = std::path::Path::new(&file).exists();
        match storage.load() {
            Ok(pantones) => {
                let mut message = None;
                if !exists {
                    message = Some(format!("Файл «{file}» не найден — создан новый"));
                }
                let mut recent = load_recent_paths();
                recent.retain(|p| p != &file);
                recent.insert(0, file.clone());
                let _ = save_recent_paths(&recent);
                (pantones, file, message)
            }
            Err(e) => {
                let fallback = load_recent_paths().into_iter().find(|p| *p != file);
                match fallback {
                    Some(alt) => match Storage::new(&alt).load() {
                        Ok(pantones) => (
                            pantones,
                            alt.clone(),
                            Some(format!(
                                "«{file}» не открылся: {e}. Открыт последний: {alt}"
                            )),
                        ),
                        Err(_) => (
                            Vec::new(),
                            file.clone(),
                            Some(format!("Не удалось открыть «{file}»: {e}")),
                        ),
                    },
                    None => (
                        Vec::new(),
                        file.clone(),
                        Some(format!("Не удалось открыть «{file}»: {e}")),
                    ),
                }
            }
        }
    }

    fn recompute_visible(&mut self) {
        let q = self.filter.trim().to_lowercase();
        self.visible = if q.is_empty() {
            (0..self.pantones.len()).collect()
        } else {
            self.pantones
                .iter()
                .enumerate()
                .filter(|(_, p)| p.number.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect()
        };
        // Apply current sort to the filtered visible slice
        self.apply_sort();
    }

    fn cycle_sort(&mut self) {
        self.sort_mode = match self.sort_mode {
            SortMode::None => SortMode::NumberAsc,
            SortMode::NumberAsc => SortMode::NumberDesc,
            SortMode::NumberDesc => SortMode::WeightAsc,
            SortMode::WeightAsc => SortMode::WeightDesc,
            SortMode::WeightDesc => SortMode::None,
        };
    }

    fn apply_sort(&mut self) {
        // Sort only the currently visible indices, preserving cmap of pantones order otherwise
        // If nothing to sort, return
        if self.visible.is_empty() {
            return;
        }
        match self.sort_mode {
            SortMode::None => {
                // Do nothing
            }
            SortMode::NumberAsc => self.visible.sort_by(|&a, &b| {
                let na = &self.pantones[a].number;
                let nb = &self.pantones[b].number;
                na.to_lowercase().cmp(&nb.to_lowercase())
            }),
            SortMode::NumberDesc => self.visible.sort_by(|&a, &b| {
                let na = &self.pantones[a].number;
                let nb = &self.pantones[b].number;
                nb.to_lowercase().cmp(&na.to_lowercase())
            }),
            SortMode::WeightAsc => self.visible.sort_by(|&a, &b| {
                self.pantones[a]
                    .weight_kg
                    .partial_cmp(&self.pantones[b].weight_kg)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortMode::WeightDesc => self.visible.sort_by(|&a, &b| {
                self.pantones[b]
                    .weight_kg
                    .partial_cmp(&self.pantones[a].weight_kg)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
        }
    }

    fn save(&mut self) {
        match Storage::new(&self.file).save(&self.pantones) {
            Ok(()) => self.message = Some(format!("Сохранено: {}", self.file)),
            Err(e) => self.message = Some(format!("Ошибка сохранения: {e}")),
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal != Modal::None {
            return;
        }

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_native();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.open_add();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
            if let Some(sel) = self.selected {
                if sel < self.pantones.len() {
                    self.open_edit(Some(sel));
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
            if ctx.memory(|m| m.focused()).is_none() && self.selected.is_some() {
                self.open_delete();
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.cycle_sort();
            self.recompute_visible();
        }
    }

    fn do_open(&mut self, path: String) {
        let path = path.trim().to_string();
        if path.is_empty() {
            self.message = Some("Укажите путь к файлу".into());
            return;
        }
        match Storage::new(&path).load() {
            Ok(pantones) => {
                self.pantones = pantones;
                self.file = path.clone();
                self.filter.clear();
                self.recompute_visible();
                self.selected = None;
                let mut recent = load_recent_paths();
                recent.retain(|p| p != &path);
                recent.insert(0, path.clone());
                let _ = save_recent_paths(&recent);
                self.message = Some(format!("Открыто: {path}"));
                self.close_modal();
            }
            Err(e) => {
                self.message = Some(format!("Не удалось открыть «{path}»: {e}"));
            }
        }
    }

    fn open_add(&mut self) {
        self.form_number = "PANTONE ".into();
        self.form_r.clear();
        self.form_g.clear();
        self.form_b.clear();
        self.form_weight = "0".into();
        self.form_error = None;
        self.filter.clear();
        self.recompute_visible();
        self.focus_form_first = true;
        self.modal = Modal::Add;
    }

    fn open_edit(&mut self, idx: Option<usize>) {
        let idx = match idx {
            Some(i) if i < self.pantones.len() => i,
            _ => return,
        };
        let p = &self.pantones[idx];
        self.form_number = p.number.clone();
        self.form_r = p.rgb_r.to_string();
        self.form_g = p.rgb_g.to_string();
        self.form_b = p.rgb_b.to_string();
        self.form_weight = p.weight_kg.to_string();
        self.form_error = None;
        self.selected = Some(idx);
        self.focus_form_first = true;
        self.modal = Modal::Edit(idx);
    }

    fn open_delete(&mut self) {
        if let Some(i) = self.selected {
            if i < self.pantones.len() {
                self.form_error = None;
                self.modal = Modal::Delete(i);
            }
        }
    }

    fn open_native(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("CSV-файлы", &["csv"])
            .add_filter("Все файлы", &["*"]);
        if let Some(parent) = std::path::Path::new(&self.file).parent() {
            if parent.exists() {
                dialog = dialog.set_directory(parent);
            }
        }
        if let Some(picked) = dialog.pick_file() {
            let path = picked.to_string_lossy().to_string();
            if !path.is_empty() {
                self.do_open(path);
            }
        }
    }

    fn close_modal(&mut self) {
        self.modal = Modal::None;
        self.form_number.clear();
        self.form_r.clear();
        self.form_g.clear();
        self.form_b.clear();
        self.form_weight.clear();
        self.form_error = None;
    }

    fn submit_form(&mut self) {
        let edit_index = match self.modal {
            Modal::Edit(i) => Some(i),
            _ => None,
        };

        let number = self.form_number.trim().to_string();
        if number.is_empty() {
            self.form_error = Some("Укажите номер пантона".into());
            return;
        }
        let rgb_strs = [&self.form_r, &self.form_g, &self.form_b];
        if rgb_strs.iter().any(|s| s.trim().is_empty()) {
            self.form_error =
                Some("Цвет не задан: заполните Красный, Зелёный и Синий (0-255)".into());
            return;
        }
        let labels = ["Красный", "Зелёный", "Синий"];
        let mut rgb = [0u8; 3];
        for (slot, (field, label)) in rgb.iter_mut().zip(rgb_strs.iter().zip(labels)) {
            match field.trim().parse::<u8>() {
                Ok(v) => *slot = v,
                Err(_) => {
                    self.form_error = Some(format!("«{label}» должно быть 0-255"));
                    return;
                }
            }
        }
        if rgb == [0, 0, 0] {
            self.form_error =
                Some("Цвет не задан: Красный, Зелёный и Синий не могут быть все 0".into());
            return;
        }
        let weight: f64 = match self.form_weight.trim().replace(',', ".").parse() {
            Ok(w) if w >= 0.0 => w,
            _ => {
                self.form_error = Some("Вес должен быть неотрицательным числом".into());
                return;
            }
        };
        let duplicate = self.pantones.iter().enumerate().any(|(i, p)| {
            p.number.to_lowercase() == number.to_lowercase() && Some(i) != edit_index
        });
        if duplicate {
            self.form_error = Some(format!("Пантон «{number}» уже существует"));
            return;
        }

        let new_pantone = Pantone::new(number.clone(), rgb[0], rgb[1], rgb[2], weight);
        match edit_index {
            Some(i) => {
                self.pantones[i] = new_pantone;
                self.message = Some(format!("Обновлено: «{number}»"));
            }
            None => {
                self.pantones.push(new_pantone);
                self.selected = Some(self.pantones.len() - 1);
                self.message = Some(format!("Добавлено: «{number}»"));
            }
        }
        self.filter.clear();
        self.recompute_visible();
        match Storage::new(&self.file).save(&self.pantones) {
            Ok(()) => {}
            Err(e) => self.message = Some(format!("Ошибка сохранения: {e}")),
        }
        self.close_modal();
    }

    fn confirm_delete(&mut self) {
        if let Modal::Delete(i) = self.modal {
            if i < self.pantones.len() {
                let removed = self.pantones.remove(i);
                match self.selected {
                    Some(s) if s == i => {
                        self.selected = if self.pantones.is_empty() {
                            None
                        } else {
                            Some(i.min(self.pantones.len() - 1))
                        }
                    }
                    Some(s) if s > i => self.selected = Some(s - 1),
                    _ => {}
                }
                self.message = Some(format!("Удалено: «{}»", removed.number));
                self.recompute_visible();
                match Storage::new(&self.file).save(&self.pantones) {
                    Ok(()) => {}
                    Err(e) => self.message = Some(format!("Ошибка сохранения: {e}")),
                }
            }
        }
        self.close_modal();
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Открыть…").on_hover_text("Ctrl+O").clicked() {
                self.open_native();
            }
            let recent = load_recent_paths();
            ui.menu_button("Недавние", |ui| {
                if recent.is_empty() {
                    ui.weak("(нет истории)");
                }
                for r in &recent {
                    if ui.button(r).clicked() {
                        let path = r.clone();
                        self.do_open(path);
                        ui.close();
                    }
                }
            });
            if ui.button("Сохранить").on_hover_text("Ctrl+S").clicked() {
                self.save();
            }
            ui.separator();
            // Cyclic sort button (None/NumberAsc/NumberDesc/WeightAsc/WeightDesc)
            let sort_label = match self.sort_mode {
                SortMode::None => "Сортировка: None",
                SortMode::NumberAsc => "Сортировка: № (A)",
                SortMode::NumberDesc => "Сортировка: № (Z)",
                SortMode::WeightAsc => "Сортировка: Вес ↑",
                SortMode::WeightDesc => "Сортировка: Вес ↓",
            };
            if ui.button(sort_label).on_hover_text("F5").clicked() {
                self.cycle_sort();
                self.recompute_visible();
            }
            ui.separator();
            if ui.button("Добавить").on_hover_text("Ctrl+N").clicked() {
                self.open_add();
            }
            let can_edit = self.selected.is_some() && !self.pantones.is_empty();
            if ui
                .add_enabled(can_edit, egui::Button::new("Редактировать"))
                .on_hover_text("F2")
                .clicked()
            {
                self.open_edit(self.selected);
            }
            if ui
                .add_enabled(can_edit, egui::Button::new("Удалить"))
                .on_hover_text("Delete")
                .clicked()
            {
                self.open_delete();
            }
            ui.separator();

            ui.label("Поиск:");
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("номер пантона… (Ctrl+F)")
                    .desired_width(220.0),
            );
            if self.focus_search {
                search_resp.request_focus();
                self.focus_search = false;
            }
            if !self.filter.is_empty() && ui.small_button("Сброс").clicked() {
                self.filter.clear();
            }
            self.recompute_visible();
            ui.separator();

            let count = if self.visible.len() == self.pantones.len() {
                format!("Всего: {} шт.", self.pantones.len())
            } else {
                format!(
                    "Показано: {}/{} шт.",
                    self.visible.len(),
                    self.pantones.len()
                )
            };
            ui.label(RichText::new(count).color(GRAY).weak());
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let text = match &self.message {
                Some(m) => RichText::new(m).color(ACCENT),
                None => RichText::new(&self.file).color(FAINT),
            };
            ui.label(text);
        });
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let mut clicked: Option<usize> = None;
        let mut double: Option<usize> = None;
        let pantones = &self.pantones;
        let visible = &self.visible;
        let selected = self.selected;

        TableBuilder::new(ui)
            .id_salt("pantones_table")
            .striped(true)
            .resizable(true)
            .vscroll(true)
            .sense(egui::Sense::click())
            .column(Column::auto().at_least(44.0))
            .column(Column::initial(260.0).at_least(120.0).clip(true))
            .column(Column::auto().at_least(90.0))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::remainder().at_least(60.0))
            .header(26.0, |mut header| {
                for title in ["Образец", "Цвет Pantone", "CMYK", "HTML", "RGB", "Вес"]
                {
                    header.col(|ui| {
                        ui.label(RichText::new(title).strong().color(ACCENT));
                    });
                }
            })
            .body(|body| {
                body.rows(24.0, visible.len(), |mut row| {
                    let gi = visible[row.index()];
                    let p = &pantones[gi];
                    row.set_selected(selected == Some(gi));

                    row.col(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
                        let color = Color32::from_rgb(p.rgb_r, p.rgb_g, p.rgb_b);
                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(3), color);
                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(3),
                            egui::Stroke::new(1.0, FAINT),
                            egui::StrokeKind::Inside,
                        );
                    });

                    row.col(|ui| {
                        ui.label(RichText::new(&p.number).strong());
                    });
                    row.col(|ui| {
                        ui.label(format!("{}", p.cmyk()));
                    });
                    row.col(|ui| {
                        ui.label(p.html());
                    });
                    row.col(|ui| {
                        ui.label(p.rgb_string());
                    });
                    row.col(|ui| {
                        ui.label(format_weight(p.weight_kg));
                    });

                    let resp = row.response();
                    if resp.double_clicked() {
                        double = Some(gi);
                    } else if resp.clicked() {
                        clicked = Some(gi);
                    }
                });
            });

        if let Some(i) = double {
            self.open_edit(Some(i));
        } else if let Some(i) = clicked {
            self.selected = Some(i);
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        if self.pantones.is_empty() && self.modal == Modal::None {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Нет данных. Добавьте пантон или откройте файл.").color(GRAY),
                );
            });
        } else if self.visible.is_empty() && self.modal == Modal::None {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Ничего не найдено по запросу «{}»",
                        self.filter.trim()
                    ))
                    .color(GRAY),
                );
            });
        } else {
            self.table(ui);
        }
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        match self.modal {
            Modal::None => {}
            Modal::Add | Modal::Edit(_) => self.draw_form_modal(ctx),
            Modal::Delete(i) => self.draw_delete_modal(ctx, i),
        }
    }

    fn draw_form_modal(&mut self, ctx: &egui::Context) {
        let editing = matches!(self.modal, Modal::Edit(_));

        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        egui::Modal::new(egui::Id::new("form_modal"))
            .backdrop_color(Color32::from_black_alpha(140))
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                let title = if editing {
                    "Изменение пантона"
                } else {
                    "Новый пантон"
                };
                ui.heading(RichText::new(title).color(ACCENT));
                ui.add_space(10.0);

                egui::Grid::new("form_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Номер:");
                        let num_resp = ui.text_edit_singleline(&mut self.form_number);
                        if self.focus_form_first {
                            num_resp.request_focus();
                            self.focus_form_first = false;
                        }
                        ui.end_row();

                        ui.label("Красный (0-255):");
                        ui.text_edit_singleline(&mut self.form_r);
                        ui.end_row();

                        ui.label("Зелёный (0-255):");
                        ui.text_edit_singleline(&mut self.form_g);
                        ui.end_row();

                        ui.label("Синий (0-255):");
                        ui.text_edit_singleline(&mut self.form_b);
                        ui.end_row();

                        ui.label("Вес, кг:");
                        ui.text_edit_singleline(&mut self.form_weight);
                        ui.end_row();
                    });

                if let Some(err) = &self.form_error {
                    ui.add_space(8.0);
                    ui.colored_label(ERROR, err);
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("ОК").clicked() {
                        self.submit_form();
                    }
                    if ui.button("Отмена").clicked() {
                        self.close_modal();
                    }
                });
            });

        if escape {
            self.close_modal();
        }
    }

    fn draw_delete_modal(&mut self, ctx: &egui::Context, idx: usize) {
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let label = if idx < self.pantones.len() {
            format!("Удалить «{}» безвозвратно?", self.pantones[idx].number)
        } else {
            "Запись не найдена".to_string()
        };

        egui::Modal::new(egui::Id::new("delete_modal"))
            .backdrop_color(Color32::from_black_alpha(140))
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.heading(RichText::new("Удаление").color(ACCENT));
                ui.add_space(8.0);
                ui.label(label);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Удалить").fill(ERROR)).clicked() {
                        self.confirm_delete();
                    }
                    if ui.button("Отмена").clicked() {
                        self.close_modal();
                    }
                });
            });

        if escape {
            self.close_modal();
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keyboard shortcuts for the main window
        self.handle_shortcuts(ctx);

        // Enter submits the Add/Edit form
        if matches!(self.modal, Modal::Add) || matches!(self.modal, Modal::Edit(_)) {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                self.submit_form();
            }
        }

        if let Some(dropped) =
            ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()))
        {
            let path = dropped.to_string_lossy().to_string();
            if path.to_lowercase().ends_with(".csv") {
                self.do_open(path);
            } else {
                self.message = Some("Перетащите CSV-файл в окно".into());
            }
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| self.toolbar(ui));

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(8, 8)),
            )
            .show(ctx, |ui| self.central(ui));

        self.draw_modal(ctx);
    }
}

fn theme() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = PANEL_HI;
    v.override_text_color = Some(TEXT);

    v.selection.bg_fill = SELECT_BG;
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive] {
        w.bg_fill = PANEL_HI;
        w.bg_stroke = egui::Stroke::new(1.0, FAINT);
        w.fg_stroke = egui::Stroke::new(1.0, TEXT);
    }
    let hovered = &mut v.widgets.hovered;
    hovered.bg_fill = Color32::from_rgb(44, 50, 68);
    hovered.fg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(220, 226, 236));
    let active = &mut v.widgets.active;
    active.bg_fill = SELECT_BG;
    active.fg_stroke = egui::Stroke::new(1.0, ACCENT);

    v
}

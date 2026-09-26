use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, HighlightSpacing, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
};

use pantones_core::model::{Pantone, format_weight};
use pantones_core::storage::Storage;

const MESSAGE_TICKS: u16 = 120;
const MAX_RECENT: usize = 10;

// ---------- theme ----------
const BG: Color = Color::Rgb(19, 22, 30);
const PANEL: Color = Color::Rgb(26, 30, 41);
const PANEL_HI: Color = Color::Rgb(34, 39, 53);
const ACCENT: Color = Color::LightCyan;
const ACCENT_DIM: Color = Color::Rgb(130, 178, 190);
const MUTED: Color = Color::Rgb(120, 130, 150);
const FAINT: Color = Color::Rgb(84, 92, 112);
const SELECT_BG: Color = Color::Rgb(52, 62, 92);
const GRAY_TEXT: Color = Color::Rgb(158, 165, 181);
const SOFT_BLUE: Color = Color::Rgb(116, 180, 255);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    Add,
    Edit,
    Search,
    ConfirmDelete,
    OpenFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Int,
    Float,
}

struct Field {
    label: &'static str,
    value: String,
    cursor: usize,
    kind: FieldKind,
}

impl Field {
    fn text(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: value.to_string(),
            cursor: value.chars().count(),
            kind: FieldKind::Text,
        }
    }

    fn int(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: value.to_string(),
            cursor: value.chars().count(),
            kind: FieldKind::Int,
        }
    }

    fn float(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: value.to_string(),
            cursor: value.chars().count(),
            kind: FieldKind::Float,
        }
    }
}

pub struct App {
    storage: Storage,
    pantones: Vec<Pantone>,
    filter: String,
    visible: Vec<usize>,
    selected: usize,
    table_state: TableState,
    mode: Mode,
    fields: Vec<Field>,
    active_field: usize,
    pending_delete: Option<usize>,
    form_error: Option<String>,
    message: Option<String>,
    message_is_error: bool,
    message_ticks: u16,
    known_files: Vec<String>,
    open_path: String,
    open_error: Option<String>,
    open_zone: bool,
    selected_recent: usize,
}

impl App {
    pub fn new(path: impl Into<String>) -> Result<Self, String> {
        let file_path = path.into();
        let storage = Storage::new(&file_path);
        let pantones = storage.load()?;

        let mut known_files = pantones_core::storage::load_recent_paths();
        known_files.retain(|p| p != &file_path);
        known_files.insert(0, file_path.clone());
        known_files.truncate(MAX_RECENT);
        let _ = pantones_core::storage::save_recent_paths(&known_files);

        let mut app = Self {
            storage,
            pantones,
            filter: String::new(),
            visible: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            mode: Mode::Browse,
            fields: Vec::new(),
            active_field: 0,
            pending_delete: None,
            form_error: None,
            message: None,
            message_is_error: false,
            message_ticks: 0,
            known_files,
            open_path: String::new(),
            open_error: None,
            open_zone: true,
            selected_recent: 0,
        };
        app.recompute_visible();
        Ok(app)
    }

    pub fn tick(&mut self) {
        if self.message.is_some() {
            self.message_ticks += 1;
            if self.message_ticks >= MESSAGE_TICKS {
                self.message = None;
                self.message_ticks = 0;
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Block::default().style(Style::default().bg(BG)),
            frame.area(),
        );

        let vertical = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ]);
        let [header_area, main_area, footer_area] = vertical.areas(frame.area());

        self.draw_header(frame, header_area);
        self.draw_table(frame, main_area);
        self.draw_footer(frame, footer_area);

        match self.mode {
            Mode::Browse => {}
            Mode::Add | Mode::Edit => self.draw_form(frame),
            Mode::Search => self.draw_search(frame),
            Mode::ConfirmDelete => self.draw_confirm(frame),
            Mode::OpenFile => self.draw_open_file(frame),
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(Span::styled(
            "Pantone Warehouse",
            Style::default().fg(ACCENT).bold(),
        ));
        let path = Line::from(Span::styled(
            format!("file: {}", self.storage.path),
            Style::default().fg(FAINT),
        ));

        let total: f64 = self
            .visible
            .iter()
            .map(|&i| self.pantones[i].weight_kg)
            .sum();
        let count = if self.filter.trim().is_empty() {
            self.pantones.len().to_string()
        } else {
            format!("{} of {}", self.visible.len(), self.pantones.len())
        };
        let stats = Line::from(vec![
            Span::styled(format!("{count} items"), Style::default().fg(GRAY_TEXT)),
            Span::raw("   "),
            Span::styled("total ", Style::default().fg(FAINT)),
            Span::styled(format_weight(total), Style::default().fg(ACCENT_DIM).bold()),
        ]);

        let mut lines = vec![title, path, stats];
        match &self.message {
            Some(msg) => {
                let color = if self.message_is_error {
                    Color::LightRed
                } else {
                    Color::Rgb(110, 210, 150)
                };
                lines.push(Line::from(Span::styled(
                    format!("* {msg}"),
                    Style::new().fg(color).bold(),
                )));
            }
            None => lines.push(Line::from("")),
        }

        let block = Block::bordered()
            .border_style(Style::default().fg(FAINT))
            .style(Style::default().bg(PANEL));
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_table(&mut self, frame: &mut Frame, area: Rect) {
        let [table_area, scroll_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(area);

        let block = Block::bordered()
            .border_style(Style::default().fg(FAINT))
            .title(Span::styled(
                "Pantone stock",
                Style::default().fg(ACCENT).bold(),
            ))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(table_area);
        frame.render_widget(block, table_area);

        if self.visible.is_empty() {
            let hint = if self.filter.trim().is_empty() {
                "No pantones stored. Press 'a' to add one."
            } else {
                "No matches for the search. Press Esc to clear."
            };
            frame.render_widget(
                Paragraph::new(hint)
                    .style(Style::default().fg(FAINT))
                    .alignment(Alignment::Center),
                inner,
            );
            return;
        }

        if self.selected >= self.visible.len() {
            self.selected = self.visible.len() - 1;
        }

        let header = Row::new(vec![
            Cell::from("Образец"),
            Cell::from("Цвет Pantone"),
            Cell::from("CMYK"),
            Cell::from("HTML"),
            Cell::from("RGB"),
            Cell::from(Line::from("Вес").alignment(Alignment::Right)),
        ])
        .style(
            Style::default()
                .fg(ACCENT)
                .bg(PANEL_HI)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

        let rows: Vec<Row> = self
            .visible
            .iter()
            .map(|&i| {
                let p = &self.pantones[i];
                let swatch = Cell::from(Span::styled(
                    "          ",
                    Style::default().bg(Color::Rgb(p.rgb_r, p.rgb_g, p.rgb_b)),
                ));
                let cmyk = p.cmyk().to_string();
                let html = p.html();
                let rgb = p.rgb_string();
                let weight = format_weight(p.weight_kg);
                Row::new(vec![
                    swatch,
                    Cell::from(p.number.clone()).style(Style::default().fg(Color::White)),
                    Cell::from(Line::from(Span::styled(
                        cmyk,
                        Style::default().fg(GRAY_TEXT),
                    ))),
                    Cell::from(Line::from(Span::styled(
                        html,
                        Style::default().fg(SOFT_BLUE),
                    ))),
                    Cell::from(rgb).style(Style::default().fg(GRAY_TEXT)),
                    Cell::from(Line::from(weight).alignment(Alignment::Right))
                        .style(Style::default().fg(ACCENT_DIM)),
                ])
                .height(1)
            })
            .collect();

        let widths = [
            Constraint::Length(10),
            Constraint::Fill(1),
            Constraint::Length(13),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(9),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .highlight_symbol(Text::from(Span::styled("▌", Style::default().fg(ACCENT))))
            .highlight_spacing(HighlightSpacing::Always)
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .column_spacing(10);

        self.table_state.select(Some(self.selected));
        frame.render_stateful_widget(table, inner, &mut self.table_state);

        let mut scroll_state = ScrollbarState::new(self.visible.len()).position(self.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("▏"))
            .thumb_symbol("█")
            .style(Style::default().fg(FAINT));
        frame.render_stateful_widget(scrollbar, scroll_area, &mut scroll_state);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let [hints_area, mode_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(14)]).areas(area);

        let hints = match self.mode {
            Mode::Browse => {
                "j/k or arrows move   a add   e edit   d delete   / search   o open   s save   q quit"
            }
            Mode::Add | Mode::Edit => {
                "Tab/↓ next field   Shift+Tab/↑ prev field   Enter save   Esc cancel"
            }
            Mode::Search => "type to filter   Enter apply   Esc clear and close",
            Mode::ConfirmDelete => "y yes   n no / Esc cancel",
            Mode::OpenFile => "type path   Tab switch to list   Enter open   Esc cancel",
        };
        frame.render_widget(
            Paragraph::new(Span::styled(hints, Style::default().fg(FAINT))),
            hints_area,
        );

        let mode_name = match self.mode {
            Mode::Browse => "BROWSE",
            Mode::Add => "ADD",
            Mode::Edit => "EDIT",
            Mode::Search => "SEARCH",
            Mode::ConfirmDelete => "CONFIRM",
            Mode::OpenFile => "OPEN FILE",
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {mode_name} "),
                Style::default().fg(BG).bg(ACCENT).bold(),
            ))
            .alignment(Alignment::Right),
            mode_area,
        );
    }

    fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
        let width = width.min(area.width);
        let height = height.min(area.height);
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        )
    }

    fn draw_search(&self, frame: &mut Frame) {
        let area = Self::centered_rect(46, 3, frame.area());
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT_DIM))
            .title(Span::styled("Поиск", Style::default().fg(ACCENT).bold()))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let inner_w = inner.width.saturating_sub(2);
        let text = format!("{}█", self.filter);
        frame.render_widget(
            Paragraph::new(Span::styled(
                text.as_str(),
                Style::default().fg(Color::White),
            )),
            Rect::new(inner.x + 1, inner.y, inner_w, 1),
        );
        let cursor_col = inner.x.saturating_add(1).saturating_add(
            (self
                .filter
                .chars()
                .count()
                .min(inner_w.saturating_sub(1) as usize)) as u16,
        );
        frame.set_cursor_position((cursor_col, inner.y));
    }

    fn draw_form(&mut self, frame: &mut Frame) {
        let title = if self.mode == Mode::Add {
            "Add Pantone"
        } else {
            "Edit Pantone"
        };
        let extra = if self.form_error.is_some() { 2 } else { 0 };
        let height = self.fields.len() as u16 + 6 + extra;
        let area = Self::centered_rect(54, height, frame.area());

        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT_DIM))
            .title(Span::styled(title, Style::default().fg(ACCENT).bold()))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut y = inner.y + 1;

        if let Some(err) = &self.form_error {
            frame.render_widget(
                Paragraph::new(err.as_str())
                    .style(Style::default().fg(Color::LightRed).bold())
                    .wrap(Wrap { trim: true }),
                Rect::new(inner.x, y, inner.width, 2),
            );
            y += 2;
        }

        let label_w = self
            .fields
            .iter()
            .map(|f| f.label.chars().count())
            .max()
            .unwrap_or(0) as u16;
        let value_w = inner.width.saturating_sub(label_w + 2);

        for (i, field) in self.fields.iter().enumerate() {
            let label_style = if i == self.active_field {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(MUTED)
            };
            frame.render_widget(
                Paragraph::new(field.label)
                    .style(label_style)
                    .alignment(Alignment::Right),
                Rect::new(inner.x, y, label_w, 1),
            );

            let value_x = inner.x + label_w + 2;
            let value_style = if i == self.active_field {
                Style::default().fg(Color::White).bg(PANEL_HI)
            } else {
                Style::default().fg(GRAY_TEXT)
            };
            let shown: String = field.value.chars().take(value_w as usize).collect();
            frame.render_widget(
                Paragraph::new(shown.clone()).style(value_style),
                Rect::new(value_x, y, value_w, 1),
            );

            if i == self.active_field {
                let cursor_col = value_x + (field.cursor.min(shown.chars().count()) as u16);
                frame.set_cursor_position((cursor_col, y));
            }
            y += 1;
        }
    }

    fn draw_confirm(&self, frame: &mut Frame) {
        let Some(idx) = self.pending_delete else {
            return;
        };
        let Some(p) = self.pantones.get(idx) else {
            return;
        };
        let area = Self::centered_rect(64, 5, frame.area());
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightRed))
            .title(Span::styled(
                "Confirm delete",
                Style::default().fg(Color::LightRed).bold(),
            ))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(format!(
                "Delete '{}' ({} , {})?  [y/n]",
                p.number,
                p.html(),
                format_weight(p.weight_kg)
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }

    fn draw_open_file(&self, frame: &mut Frame) {
        let list_rows = if self.known_files.is_empty() {
            1
        } else {
            self.known_files.len().min(8)
        };
        let err_rows = if self.open_error.is_some() { 2 } else { 0 };
        let height = (4 + err_rows + list_rows) as u16;
        let area = Self::centered_rect(62, height, frame.area());
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT_DIM))
            .title(Span::styled(
                "Open file (CSV)",
                Style::default().fg(ACCENT).bold(),
            ))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut y = inner.y + 1;

        let path_style = if self.open_zone {
            Style::default().fg(Color::White).bg(PANEL_HI)
        } else {
            Style::default().fg(GRAY_TEXT)
        };
        frame.render_widget(
            Paragraph::new("Path:").style(path_style),
            Rect::new(inner.x, y, 6, 1),
        );
        let input_w = inner.width.saturating_sub(7) as usize;
        let shown: String = self.open_path.chars().take(input_w).collect();
        frame.render_widget(
            Paragraph::new(shown.clone()).style(path_style),
            Rect::new(inner.x + 6, y, inner.width.saturating_sub(6), 1),
        );
        if self.open_zone {
            let cursor_col = (inner.x + 6)
                .saturating_add(shown.chars().count().min(input_w.saturating_sub(1)) as u16)
                .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
            frame.set_cursor_position((cursor_col, y));
        }
        y += 1;

        if let Some(err) = &self.open_error {
            frame.render_widget(
                Paragraph::new(err.as_str())
                    .style(Style::default().fg(Color::LightRed))
                    .wrap(Wrap { trim: true }),
                Rect::new(inner.x, y, inner.width, 2),
            );
            y += 2;
        }
        y += 1;

        frame.render_widget(
            Paragraph::new(Span::styled(
                "Recent files:",
                Style::default().fg(ACCENT_DIM).bold(),
            )),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += 1;

        if self.known_files.is_empty() {
            frame.render_widget(
                Paragraph::new("(none yet)").style(Style::default().fg(FAINT)),
                Rect::new(inner.x, y, inner.width, 1),
            );
            return;
        }
        for (i, path) in self.known_files.iter().take(8).enumerate() {
            let style = if i == self.selected_recent {
                Style::default()
                    .fg(Color::White)
                    .bg(SELECT_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(GRAY_TEXT)
            };
            let short: String = path.chars().take(inner.width as usize).collect();
            frame.render_widget(
                Paragraph::new(short).style(style),
                Rect::new(inner.x, y, inner.width, 1),
            );
            y += 1;
        }
    }

    pub fn handle_event(&mut self, event: crossterm::event::Event) -> io::Result<bool> {
        if let crossterm::event::Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
            {
                return Ok(true);
            }
            match self.mode {
                Mode::Browse => return self.handle_browse_key(key),
                Mode::Add | Mode::Edit => self.handle_form_key(key)?,
                Mode::Search => self.handle_search_key(key)?,
                Mode::ConfirmDelete => self.handle_confirm_key(key)?,
                Mode::OpenFile => self.handle_open_key(key)?,
            };
        }
        Ok(false)
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Esc => {}
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Char('g') => self.selected = 0,
            KeyCode::Char('G') => self.selected = self.visible.len().saturating_sub(1),
            KeyCode::Char('a') => self.start_add(),
            KeyCode::Char('e') => self.start_edit(),
            KeyCode::Char('d') => {
                if let Some(idx) = self.current_global() {
                    self.pending_delete = Some(idx);
                    self.mode = Mode::ConfirmDelete;
                }
            }
            KeyCode::Char('/') | KeyCode::Char('f') => self.mode = Mode::Search,
            KeyCode::Char('o') => self.start_open(),
            KeyCode::Char('r') => match self.storage.load() {
                Ok(list) => {
                    let changed = list.len() != self.pantones.len();
                    self.pantones = list;
                    self.recompute_visible();
                    self.set_message(
                        if changed {
                            "Reloaded from disk".to_string()
                        } else {
                            "Reloaded from disk (unchanged)".to_string()
                        },
                        false,
                    );
                }
                Err(e) => self.set_message(format!("Reload failed: {e}"), true),
            },
            KeyCode::Char('s') => match self.storage.save(&self.pantones) {
                Ok(()) => self.set_message("Saved to disk".to_string(), false),
                Err(e) => self.set_message(format!("Save failed: {e}"), true),
            },
            _ => {}
        }
        Ok(false)
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Enter => self.mode = Mode::Browse,
            KeyCode::Esc => {
                self.filter.clear();
                self.recompute_visible();
                self.mode = Mode::Browse;
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.recompute_visible();
            }
            KeyCode::Char(ch) => {
                self.filter.push(ch);
                self.recompute_visible();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Esc => self.cancel_form(),
            KeyCode::Enter => self.submit_form_step(),
            KeyCode::Tab | KeyCode::Down => {
                if self.active_field + 1 < self.fields.len() {
                    self.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.active_field = self.active_field.saturating_sub(1)
            }
            KeyCode::Backspace => {
                let field = &mut self.fields[self.active_field];
                if field.cursor > 0 {
                    field.cursor -= 1;
                    field.value.remove(field.cursor);
                }
                self.form_error = None;
            }
            KeyCode::Delete => {
                let field = &mut self.fields[self.active_field];
                if field.cursor < field.value.chars().count() {
                    field.value.remove(field.cursor);
                }
            }
            KeyCode::Left => {
                let field = &mut self.fields[self.active_field];
                field.cursor = field.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let field = &mut self.fields[self.active_field];
                if field.cursor < field.value.chars().count() {
                    field.cursor += 1;
                }
            }
            KeyCode::Home => self.fields[self.active_field].cursor = 0,
            KeyCode::End => {
                self.fields[self.active_field].cursor =
                    self.fields[self.active_field].value.chars().count();
            }
            KeyCode::Char(ch) => {
                let field = &mut self.fields[self.active_field];
                let allowed = match field.kind {
                    FieldKind::Text => true,
                    FieldKind::Int => ch.is_ascii_digit(),
                    FieldKind::Float => ch.is_ascii_digit() || ch == '.' || ch == ',',
                };
                if !allowed {
                    self.form_error = Some(match field.kind {
                        FieldKind::Int => "RGB value must be an integer 0-255".to_string(),
                        FieldKind::Float => "Weight must be a number".to_string(),
                        FieldKind::Text => String::new(),
                    });
                    return Ok(false);
                }
                field.value.insert(field.cursor, ch);
                field.cursor += 1;
                self.form_error = None;
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.cancel_delete(),
            _ => {}
        }
        Ok(false)
    }

    fn start_open(&mut self) {
        self.open_path = self.storage.path.clone();
        self.open_error = None;
        self.open_zone = true;
        self.selected_recent = 0;
        self.mode = Mode::OpenFile;
    }

    fn handle_open_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                self.open_error = None;
            }
            KeyCode::Tab | KeyCode::BackTab => self.open_zone = !self.open_zone,
            KeyCode::Enter => {
                let target = if self.open_zone {
                    self.open_path.trim().to_string()
                } else {
                    self.known_files
                        .get(self.selected_recent)
                        .cloned()
                        .unwrap_or_default()
                };
                if !target.is_empty() {
                    self.switch_to(&target);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if !self.open_zone => {
                self.selected_recent = self.selected_recent.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') if !self.open_zone => {
                if self.selected_recent + 1 < self.known_files.len() {
                    self.selected_recent += 1;
                }
            }
            KeyCode::Backspace if self.open_zone => {
                self.open_path.pop();
            }
            KeyCode::Char(ch) if self.open_zone => {
                self.open_path.push(ch);
            }
            _ => {}
        }
        Ok(false)
    }

    fn switch_to(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        match Storage::new(path).load() {
            Ok(list) => {
                self.storage = Storage::new(path);
                self.pantones = list;
                self.filter.clear();
                self.recompute_visible();
                self.selected = 0;
                let path_string = path.to_string();
                self.known_files.retain(|p| p != &path_string);
                self.known_files.insert(0, path_string.clone());
                self.known_files.truncate(MAX_RECENT);
                let _ = pantones_core::storage::save_recent_paths(&self.known_files);
                self.set_message(format!("Opened: {path_string}"), false);
                self.mode = Mode::Browse;
                self.open_error = None;
            }
            Err(e) => self.open_error = Some(e),
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, self.visible.len() as isize - 1) as usize;
    }

    fn current_global(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    fn recompute_visible(&mut self) {
        let old = self.current_global();
        let query = self.filter.trim().to_lowercase();
        self.visible = self
            .pantones
            .iter()
            .enumerate()
            .filter(|(_, p)| query.is_empty() || p.number.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        if let Some(old_idx) = old {
            if let Some(pos) = self.visible.iter().position(|&i| i == old_idx) {
                self.selected = pos;
                return;
            }
        }
        if self.selected >= self.visible.len() {
            self.selected = self.visible.len().saturating_sub(1);
        }
    }

    fn start_add(&mut self) {
        self.fields = vec![
            Field::text("Pantone number:", ""),
            Field::int("Red (0-255):", ""),
            Field::int("Green (0-255):", ""),
            Field::int("Blue (0-255):", ""),
            Field::float("Weight, kg:", "0"),
        ];
        self.active_field = 0;
        self.form_error = None;
        self.mode = Mode::Add;
    }

    fn start_edit(&mut self) {
        let Some(idx) = self.current_global() else {
            return;
        };
        let p = &self.pantones[idx];
        self.fields = vec![
            Field::text("Pantone number:", &p.number),
            Field::int("Red (0-255):", &p.rgb_r.to_string()),
            Field::int("Green (0-255):", &p.rgb_g.to_string()),
            Field::int("Blue (0-255):", &p.rgb_b.to_string()),
            Field::float("Weight, kg:", &p.weight_kg.to_string()),
        ];
        self.active_field = 0;
        self.form_error = None;
        self.mode = Mode::Edit;
    }

    fn submit_form_step(&mut self) {
        if self.active_field < self.fields.len() - 1 {
            self.active_field += 1;
            return;
        }
        match self.mode {
            Mode::Add => self.save_form(None),
            Mode::Edit => self.save_form(self.current_global()),
            Mode::Browse | Mode::Search | Mode::ConfirmDelete | Mode::OpenFile => {}
        }
    }

    fn save_form(&mut self, edit_index: Option<usize>) {
        let number = self.fields[0].value.trim().to_string();
        if number.is_empty() {
            self.form_error = Some("Pantone number cannot be empty".to_string());
            return;
        }
        let empty_ok = [&self.fields[1], &self.fields[2], &self.fields[3]];
        if empty_ok.iter().any(|f| f.value.trim().is_empty()) {
            self.form_error =
                Some("Color is not set: fill in Red, Green and Blue values (0-255)".to_string());
            return;
        }
        let mut rgb = [0u8; 3];
        for (slot, field) in rgb.iter_mut().zip(&self.fields[1..4]) {
            match field.value.trim().parse::<u8>() {
                Ok(v) => *slot = v,
                Err(_) => {
                    self.form_error = Some(format!(
                        "{} must be 0-255",
                        field.label.trim_end_matches(" (0-255):")
                    ));
                    return;
                }
            }
        }
        if rgb == [0, 0, 0] {
            self.form_error =
                Some("Color is not set: Red, Green and Blue cannot all be 0".to_string());
            return;
        }
        let raw_weight = self.fields[4].value.trim().replace(',', ".");
        let weight: f64 = match raw_weight.parse() {
            Ok(w) if w >= 0.0 => w,
            _ => {
                self.form_error = Some("Weight must be a non-negative number".to_string());
                return;
            }
        };

        let duplicate = self.pantones.iter().enumerate().any(|(i, p)| {
            p.number.to_lowercase() == number.to_lowercase() && Some(i) != edit_index
        });
        if duplicate {
            self.form_error = Some(format!("Pantone '{number}' already exists"));
            return;
        }

        let new_pantone = Pantone::new(number.clone(), rgb[0], rgb[1], rgb[2], weight);
        match edit_index {
            Some(i) => {
                self.pantones[i] = new_pantone;
                self.recompute_visible();
                self.set_message(format!("Updated '{number}'"), false);
            }
            None => {
                self.pantones.push(new_pantone);
                let target = self.pantones.len() - 1;
                self.recompute_visible();
                if let Some(pos) = self.visible.iter().position(|&i| i == target) {
                    self.selected = pos;
                }
                self.set_message(format!("Added '{number}'"), false);
            }
        }
        if let Err(e) = self.storage.save(&self.pantones) {
            self.set_message(format!("Save failed: {e}"), true);
        }
        self.cancel_form();
    }

    fn cancel_form(&mut self) {
        self.mode = Mode::Browse;
        self.fields.clear();
        self.form_error = None;
    }

    fn confirm_delete(&mut self) {
        if let Some(idx) = self.pending_delete.take() {
            if idx < self.pantones.len() {
                let removed = self.pantones.remove(idx);
                self.recompute_visible();
                self.set_message(format!("Deleted '{}'", removed.number), false);
            }
            if let Err(e) = self.storage.save(&self.pantones) {
                self.set_message(format!("Save failed: {e}"), true);
            }
        }
        self.mode = Mode::Browse;
    }

    fn cancel_delete(&mut self) {
        self.pending_delete = None;
        self.mode = Mode::Browse;
    }

    fn set_message(&mut self, message: String, is_error: bool) {
        self.message = Some(message);
        self.message_is_error = is_error;
        self.message_ticks = 0;
    }
}

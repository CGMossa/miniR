//! Interactive plot window using egui/eframe/egui_plot.
//!
//! The plot window runs on the main thread (macOS requirement). The REPL
//! sends plot data through a channel; the egui event loop picks it up and
//! renders it. The REPL never blocks.
//!
//! Architecture:
//! - Main thread: egui event loop (idles when no plots are open)
//! - Background thread: REPL + interpreter
//! - Communication: `PlotChannel` (mpsc sender/receiver)

use std::sync::mpsc;

use super::plot_data::{PlotItem, PlotState};
use super::view::TableData;

// region: PlotChannel

/// Message from the REPL thread to the GUI thread.
pub enum PlotMessage {
    /// Show a new plot (replaces the current one).
    Show(PlotState),
    /// Close the current plot window.
    Close,
    /// Show a View() data frame table.
    View(TableData),
}

/// Sender half — stored on the Interpreter so builtins can send plots.
pub type PlotSender = mpsc::Sender<PlotMessage>;

/// Receiver half — owned by the egui event loop on the main thread.
pub type PlotReceiver = mpsc::Receiver<PlotMessage>;

/// Create a connected (sender, receiver) pair.
pub fn plot_channel() -> (PlotSender, PlotReceiver) {
    mpsc::channel()
}

// endregion

// region: egui app

/// Map an R pch value (0-25) to an `egui_plot::MarkerShape`.
fn pch_to_marker(pch: u8) -> egui_plot::MarkerShape {
    match pch {
        0 => egui_plot::MarkerShape::Square,
        1 => egui_plot::MarkerShape::Circle,
        2 => egui_plot::MarkerShape::Up,
        3 => egui_plot::MarkerShape::Plus,
        4 => egui_plot::MarkerShape::Cross,
        5 => egui_plot::MarkerShape::Diamond,
        6 => egui_plot::MarkerShape::Down,
        8 => egui_plot::MarkerShape::Asterisk,
        15 => egui_plot::MarkerShape::Square,
        16 | 19 | 20 => egui_plot::MarkerShape::Circle,
        17 => egui_plot::MarkerShape::Up,
        18 => egui_plot::MarkerShape::Diamond,
        _ => egui_plot::MarkerShape::Circle,
    }
}

fn pch_is_filled(pch: u8) -> bool {
    pch >= 15
}

fn rgba_to_color32(c: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Interactive state for the plot sidebar controls.
struct PlotViewState {
    show_sidebar: bool,
    point_size: f32,
    line_width: f32,
    show_grid: bool,
    show_legend: bool,
}

impl Default for PlotViewState {
    fn default() -> Self {
        Self {
            show_sidebar: false,
            point_size: 3.0,
            line_width: 1.5,
            show_grid: true,
            show_legend: true,
        }
    }
}

/// A single tab in the GUI window.
enum Tab {
    Plot {
        title: String,
        state: PlotState,
        view_state: PlotViewState,
    },
    Table {
        title: String,
        data: TableData,
        view_state: TableViewState,
    },
}

/// Interactive state for a View() table tab.
struct TableViewState {
    filter: String,
    sort_col: Option<usize>,
    sort_desc: bool,
    digits: Option<usize>,
    visible_rows: Vec<usize>,
    dirty: bool,
    /// Selected row index in visible_rows (for highlighting + stats).
    selected_row: Option<usize>,
    /// Selected column (for summary stats).
    selected_col: Option<usize>,
    /// Column visibility (true = shown).
    col_visible: Vec<bool>,
    /// Show column visibility panel.
    show_col_picker: bool,
    /// Show floating statistics window.
    show_stats_window: bool,
}

impl TableViewState {
    fn new(ncol: usize, nrow: usize) -> Self {
        Self {
            filter: String::new(),
            sort_col: None,
            sort_desc: false,
            digits: None,
            visible_rows: (0..nrow).collect(),
            dirty: false,
            selected_row: None,
            selected_col: None,
            col_visible: vec![true; ncol],
            show_col_picker: false,
            show_stats_window: false,
        }
    }

    fn recompute(&mut self, data: &TableData) {
        let mut indices: Vec<usize> = if self.filter.is_empty() {
            (0..data.rows.len()).collect()
        } else {
            let needle = self.filter.to_lowercase();
            (0..data.rows.len())
                .filter(|&r| {
                    data.rows[r]
                        .iter()
                        .any(|cell| cell.to_lowercase().contains(&needle))
                        || data
                            .row_names
                            .get(r)
                            .is_some_and(|rn| rn.to_lowercase().contains(&needle))
                })
                .collect()
        };

        if let Some(col) = self.sort_col {
            indices.sort_by(|&a, &b| {
                let va = data.rows[a].get(col).map(|s| s.as_str()).unwrap_or("");
                let vb = data.rows[b].get(col).map(|s| s.as_str()).unwrap_or("");
                // Try numeric comparison first
                let cmp = match (va.parse::<f64>(), vb.parse::<f64>()) {
                    (Ok(fa), Ok(fb)) => fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal),
                    _ => va.cmp(vb),
                };
                if self.sort_desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            });
        }

        self.visible_rows = indices;
        self.dirty = false;
    }
}

impl Tab {
    fn title(&self) -> &str {
        match self {
            Tab::Plot { title, .. } | Tab::Table { title, .. } => title,
        }
    }
}

/// The eframe app. Manages tabbed plots from the REPL thread.
/// The window only exists while there are plots to display.
struct PlotApp {
    tabs: Vec<Tab>,
    active_tab: usize,
    rx: std::sync::Arc<std::sync::Mutex<PlotReceiver>>,
    /// Whether dark mode is active (true) or light mode (false).
    dark_mode: bool,
    /// Status message from save operations, auto-clears after 5 seconds.
    save_msg: Option<(String, std::time::Instant)>,
}

impl eframe::App for PlotApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dark_mode", &self.dark_mode);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle window close (X button). On macOS eframe may not close
        // automatically — explicitly allow it.
        // X button: hide the window instead of closing. On macOS, eframe::run_native
        // can only be called once per process, so we keep the app alive but hidden.
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.tabs.clear();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        // Keyboard shortcuts
        let close_tab = ctx.input(|i| i.key_pressed(egui::Key::W) && i.modifiers.command);
        let next_tab = ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl);
        if close_tab && !self.tabs.is_empty() {
            self.tabs.remove(self.active_tab);
            if self.tabs.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
        if next_tab && self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }

        // Check for messages from the REPL thread (non-blocking).
        let rx = self.rx.lock().unwrap();
        while let Ok(msg) = rx.try_recv() {
            let was_empty = self.tabs.is_empty();
            match msg {
                PlotMessage::Show(new_state) => {
                    let title = new_state
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("Plot {}", self.tabs.len() + 1));
                    self.tabs.push(Tab::Plot {
                        title,
                        state: new_state,
                        view_state: PlotViewState::default(),
                    });
                    self.active_tab = self.tabs.len() - 1;
                }
                PlotMessage::View(data) => {
                    let title = data.title.clone();
                    let nrow = data.rows.len();
                    self.tabs.push(Tab::Table {
                        title,
                        view_state: TableViewState::new(data.headers.len(), nrow),
                        data,
                    });
                    self.active_tab = self.tabs.len() - 1;
                }
                PlotMessage::Close => {
                    if !self.tabs.is_empty() {
                        self.tabs.remove(self.active_tab);
                        if self.active_tab > 0 {
                            self.active_tab -= 1;
                        }
                    }
                    if self.tabs.is_empty() {
                        // Hide window — it will reappear when new data arrives
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    }
                }
            }
            // Show window if it was hidden and we just added a tab
            if was_empty && !self.tabs.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        drop(rx);

        // Request a repaint periodically so we pick up new messages.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        if self.tabs.is_empty() {
            return;
        }

        // Render tab bar
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut to_close = None;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let selected = i == self.active_tab;
                    let label = if selected {
                        egui::RichText::new(tab.title()).strong()
                    } else {
                        egui::RichText::new(tab.title())
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        self.active_tab = i;
                    }
                    // Close button for each tab
                    if ui.small_button("\u{00d7}").clicked() {
                        to_close = Some(i);
                    }
                    ui.separator();
                }

                // Theme toggle button (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let icon = if self.dark_mode {
                        "\u{2600}"
                    } else {
                        "\u{1f319}"
                    };
                    let tooltip = if self.dark_mode {
                        "Switch to light mode"
                    } else {
                        "Switch to dark mode"
                    };
                    if ui
                        .add(egui::Button::new(icon).frame(false))
                        .on_hover_text(tooltip)
                        .clicked()
                    {
                        self.dark_mode = !self.dark_mode;
                        let theme = if self.dark_mode {
                            egui::ThemePreference::Dark
                        } else {
                            egui::ThemePreference::Light
                        };
                        ctx.set_theme(theme);
                    }
                });

                if let Some(idx) = to_close {
                    self.tabs.remove(idx);
                    if self.tabs.is_empty() {
                        // Hide — don't close (macOS can't reopen after close)
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    } else if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
            });
        });

        if self.tabs.is_empty() {
            return;
        }

        // Render active tab content
        let active = self.active_tab.min(self.tabs.len().saturating_sub(1));
        let save_msg = &mut self.save_msg;
        match &mut self.tabs[active] {
            Tab::Plot {
                state, view_state, ..
            } => {
                render_plot(ctx, state, view_state, save_msg);
            }
            Tab::Table {
                data, view_state, ..
            } => {
                render_table(ctx, data, view_state);
            }
        }
    }
}

/// Extract a display name for a plot series.
fn series_name(item: &PlotItem, idx: usize) -> String {
    let label = match item {
        PlotItem::Line { label, .. }
        | PlotItem::Points { label, .. }
        | PlotItem::Bars { label, .. } => label.as_deref(),
        PlotItem::BoxPlot { .. } => None,
        PlotItem::HLine { .. } => None,
        PlotItem::VLine { .. } => None,
        PlotItem::Text { text, .. } => Some(text.as_str()),
    };
    label.unwrap_or(&format!("series_{idx}")).to_string()
}

/// Extract the RGBA color for a plot series.
fn series_color(item: &PlotItem) -> [u8; 4] {
    match item {
        PlotItem::Line { color, .. }
        | PlotItem::Points { color, .. }
        | PlotItem::Bars { color, .. }
        | PlotItem::BoxPlot { color, .. }
        | PlotItem::HLine { color, .. }
        | PlotItem::VLine { color, .. }
        | PlotItem::Text { color, .. } => *color,
    }
}

/// Render a plot in the central panel with toolbar, sidebar, and context menu.
fn render_plot(
    ctx: &egui::Context,
    state: &PlotState,
    vs: &mut PlotViewState,
    save_msg: &mut Option<(String, std::time::Instant)>,
) {
    // Sidebar panel (animated show/hide)
    egui::SidePanel::left("plot_options")
        .resizable(true)
        .default_width(180.0)
        .show_animated(ctx, vs.show_sidebar, |ui| {
            ui.heading("Plot Options");
            ui.separator();

            egui::CollapsingHeader::new("Appearance")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut vs.point_size, 1.0..=20.0).text("Point size"));
                    ui.add(egui::Slider::new(&mut vs.line_width, 0.5..=10.0).text("Line width"));
                    ui.checkbox(&mut vs.show_grid, "Show grid");
                    ui.checkbox(&mut vs.show_legend, "Show legend");
                });

            egui::CollapsingHeader::new("Series")
                .default_open(true)
                .show(ui, |ui| {
                    if state.items.is_empty() {
                        ui.label(egui::RichText::new("No series").weak().italics());
                    } else {
                        for (idx, item) in state.items.iter().enumerate() {
                            let name = series_name(item, idx);
                            let rgba = series_color(item);
                            let color = rgba_to_color32(rgba);
                            ui.horizontal(|ui| {
                                let (rect, _resp) = ui.allocate_exact_size(
                                    egui::vec2(12.0, 12.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 2.0, color);
                                ui.label(&name);
                            });
                        }
                    }
                });
        });
    egui::CentralPanel::default().show(ctx, |ui| {
        let title = state.title.as_deref().unwrap_or("Plot");
        ui.heading(title);

        // Toolbar
        ui.horizontal(|ui| {
            // Sidebar toggle button
            let toggle_label = if vs.show_sidebar {
                "\u{2699} Hide Options"
            } else {
                "\u{2699} Options"
            };
            if ui.button(toggle_label).clicked() {
                vs.show_sidebar = !vs.show_sidebar;
            }

            ui.separator();

            #[cfg(feature = "svg-device")]
            if ui.button("Save SVG").clicked() {
                let path = save_path("svg");
                let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                match std::fs::write(&path, svg_str) {
                    Ok(()) => {
                        *save_msg = Some((format!("Saved: {path}"), std::time::Instant::now()))
                    }
                    Err(e) => *save_msg = Some((format!("Error: {e}"), std::time::Instant::now())),
                }
            }

            #[cfg(feature = "pdf-device")]
            if ui.button("Save PDF").clicked() {
                let path = save_path("pdf");
                let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                match super::pdf::svg_to_pdf(&svg_str, 672.0, 672.0) {
                    Ok(bytes) => match std::fs::write(&path, bytes) {
                        Ok(()) => {
                            *save_msg = Some((format!("Saved: {path}"), std::time::Instant::now()))
                        }
                        Err(e) => {
                            *save_msg = Some((format!("Error: {e}"), std::time::Instant::now()))
                        }
                    },
                    Err(e) => {
                        *save_msg = Some((format!("PDF error: {e}"), std::time::Instant::now()))
                    }
                }
            }

            ui.separator();
            ui.label(format!("{} series", state.items.len()));

            // Show save status for 5 seconds
            if let Some((text, when)) = save_msg.as_ref() {
                if when.elapsed().as_secs() < 5 {
                    ui.separator();
                    ui.label(egui::RichText::new(text).weak().italics());
                }
            }
        });

        let mut plot = egui_plot::Plot::new("r_plot")
            .show_axes(true)
            .show_grid(vs.show_grid)
            .allow_boxed_zoom(true)
            .coordinates_formatter(
                egui_plot::Corner::LeftBottom,
                egui_plot::CoordinatesFormatter::default(),
            );

        if vs.show_legend {
            plot = plot.legend(egui_plot::Legend::default());
        }

        if let Some(label) = &state.x_label {
            plot = plot.x_axis_label(label.clone());
        }
        if let Some(label) = &state.y_label {
            plot = plot.y_axis_label(label.clone());
        }
        if let Some((lo, hi)) = state.x_lim {
            plot = plot.include_x(lo).include_x(hi);
        }
        if let Some((lo, hi)) = state.y_lim {
            plot = plot.include_y(lo).include_y(hi);
        }

        let point_size = vs.point_size;
        let line_width = vs.line_width;
        let plot_response = plot.show(ui, |plot_ui| {
            for (idx, item) in state.items.iter().enumerate() {
                let default_name = format!("series_{idx}");
                render_plot_item(plot_ui, item, &default_name, idx, point_size, line_width);
            }
        });

        // Context menu on right-click anywhere in the plot area
        plot_response.response.context_menu(|ui| {
            #[cfg(feature = "svg-device")]
            if ui.button("Save SVG").clicked() {
                let path = save_path("svg");
                let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                match std::fs::write(&path, &svg_str) {
                    Ok(()) => {
                        *save_msg = Some((format!("Saved: {path}"), std::time::Instant::now()))
                    }
                    Err(e) => *save_msg = Some((format!("Error: {e}"), std::time::Instant::now())),
                }
                ui.close();
            }

            #[cfg(feature = "pdf-device")]
            if ui.button("Save PDF").clicked() {
                let path = save_path("pdf");
                let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                match super::pdf::svg_to_pdf(&svg_str, 672.0, 672.0) {
                    Ok(bytes) => match std::fs::write(&path, bytes) {
                        Ok(()) => {
                            *save_msg = Some((format!("Saved: {path}"), std::time::Instant::now()))
                        }
                        Err(e) => {
                            *save_msg = Some((format!("Error: {e}"), std::time::Instant::now()))
                        }
                    },
                    Err(e) => {
                        *save_msg = Some((format!("PDF error: {e}"), std::time::Instant::now()))
                    }
                }
                ui.close();
            }

            if ui.button("Copy coordinates").clicked() {
                if let Some(pos) = plot_response.response.hover_pos() {
                    let plot_pos = plot_response.transform.value_from_position(pos);
                    let text = format!("x={}, y={}", plot_pos.x, plot_pos.y);
                    ui.ctx().copy_text(text);
                }
                ui.close();
            }
        });
    });
}

/// Generate a save path in the current directory with a timestamp.
fn save_path(ext: &str) -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("Rplot_{ts}.{ext}")
}

fn render_plot_item(
    plot_ui: &mut egui_plot::PlotUi,
    item: &PlotItem,
    default_name: &str,
    idx: usize,
    override_point_size: f32,
    override_line_width: f32,
) {
    match item {
        PlotItem::Line {
            x, y, color, label, ..
        } => {
            let points: Vec<[f64; 2]> = x.iter().zip(y.iter()).map(|(&xi, &yi)| [xi, yi]).collect();
            let name = label.as_deref().unwrap_or(default_name);
            plot_ui.line(
                egui_plot::Line::new(name, points)
                    .color(rgba_to_color32(*color))
                    .width(override_line_width),
            );
        }
        PlotItem::Points {
            x,
            y,
            color,
            shape,
            label,
            ..
        } => {
            let points: Vec<[f64; 2]> = x.iter().zip(y.iter()).map(|(&xi, &yi)| [xi, yi]).collect();
            let name = label.as_deref().unwrap_or(default_name);
            plot_ui.points(
                egui_plot::Points::new(name, points)
                    .color(rgba_to_color32(*color))
                    .radius(override_point_size)
                    .shape(pch_to_marker(*shape))
                    .filled(pch_is_filled(*shape)),
            );
        }
        PlotItem::Bars {
            x,
            heights,
            color,
            width,
            label,
        } => {
            let bars: Vec<egui_plot::Bar> = x
                .iter()
                .zip(heights.iter())
                .map(|(&xi, &hi)| {
                    egui_plot::Bar::new(xi, hi)
                        .width(*width)
                        .fill(rgba_to_color32(*color))
                })
                .collect();
            let name = label.as_deref().unwrap_or(default_name);
            plot_ui.bar_chart(egui_plot::BarChart::new(name, bars));
        }
        PlotItem::BoxPlot {
            positions,
            spreads,
            color,
        } => {
            for (j, (pos, spread)) in positions.iter().zip(spreads.iter()).enumerate() {
                let elem = egui_plot::BoxElem::new(
                    *pos,
                    egui_plot::BoxSpread::new(
                        spread.lower_whisker,
                        spread.q1,
                        spread.median,
                        spread.q3,
                        spread.upper_whisker,
                    ),
                )
                .fill(rgba_to_color32(*color));
                plot_ui.box_plot(egui_plot::BoxPlot::new(format!("box_{j}"), vec![elem]));
            }
        }
        PlotItem::HLine { y, color, .. } => {
            plot_ui.hline(
                egui_plot::HLine::new(format!("hline_{idx}"), *y)
                    .color(rgba_to_color32(*color))
                    .width(override_line_width),
            );
        }
        PlotItem::VLine { x, color, .. } => {
            plot_ui.vline(
                egui_plot::VLine::new(format!("vline_{idx}"), *x)
                    .color(rgba_to_color32(*color))
                    .width(override_line_width),
            );
        }
        PlotItem::Text { x, y, text, color } => {
            plot_ui.text(egui_plot::Text::new(
                format!("text_{idx}"),
                egui_plot::PlotPoint::new(*x, *y),
                egui::RichText::new(text).color(rgba_to_color32(*color)),
            ));
        }
    }
}

/// Render a View() data frame table in the central panel.
fn render_table(ctx: &egui::Context, data: &TableData, vs: &mut TableViewState) {
    if vs.dirty {
        vs.recompute(data);
    }

    // Summary stats for selected column
    let summary = vs.selected_col.and_then(|col| {
        let vals: Vec<f64> = vs
            .visible_rows
            .iter()
            .filter_map(|&r| data.rows.get(r)?.get(col)?.parse::<f64>().ok())
            .collect();
        if vals.is_empty() {
            return None;
        }
        let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        Some((min, max, mean, vals.len()))
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading(&data.title);

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("🔍");
            let old_filter = vs.filter.clone();
            let filter_response = ui.add(
                egui::TextEdit::singleline(&mut vs.filter)
                    .desired_width(200.0)
                    .hint_text("Filter rows..."),
            );
            if vs.filter != old_filter {
                vs.dirty = true;
                vs.recompute(data);
            }
            // Clear filter button
            if !vs.filter.is_empty() && ui.small_button("✕").clicked() {
                vs.filter.clear();
                vs.dirty = true;
                vs.recompute(data);
                filter_response.request_focus();
            }

            ui.separator();
            ui.label("Digits:");
            let mut digits_str = vs
                .digits
                .map(|d| d.to_string())
                .unwrap_or_else(|| "-".to_string());
            let resp = ui.add(egui::TextEdit::singleline(&mut digits_str).desired_width(30.0));
            if resp.changed() {
                vs.digits = digits_str.parse::<usize>().ok();
            }

            ui.separator();
            if ui.selectable_label(vs.show_col_picker, "Columns").clicked() {
                vs.show_col_picker = !vs.show_col_picker;
            }

            ui.separator();
            ui.label(format!(
                "{}/{} rows",
                vs.visible_rows.len(),
                data.rows.len()
            ));

            // Export CSV button
            if ui.small_button("📋 CSV").clicked() {
                let mut csv = String::new();
                // Header
                csv.push_str(&data.headers.join(","));
                csv.push('\n');
                // Visible rows
                for &r in &vs.visible_rows {
                    if let Some(row) = data.rows.get(r) {
                        csv.push_str(&row.join(","));
                        csv.push('\n');
                    }
                }
                ctx.copy_text(csv);
            }
        });

        // Column visibility picker
        if vs.show_col_picker {
            ui.horizontal_wrapped(|ui| {
                for (i, header) in data.headers.iter().enumerate() {
                    let mut visible = vs.col_visible.get(i).copied().unwrap_or(true);
                    if ui.checkbox(&mut visible, header).changed() {
                        if let Some(v) = vs.col_visible.get_mut(i) {
                            *v = visible;
                        }
                    }
                }
            });
        }

        ui.separator();

        // Visible column indices
        let vis_cols: Vec<usize> = (0..data.headers.len())
            .filter(|&i| vs.col_visible.get(i).copied().unwrap_or(true))
            .collect();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("view_grid")
                    .striped(true)
                    .num_columns(vis_cols.len() + 1)
                    .min_col_width(40.0)
                    .show(ui, |ui| {
                        // Header row: column name <type>, clickable for sort
                        ui.label(egui::RichText::new("").weak()); // row name col
                        for &col_idx in &vis_cols {
                            let header = &data.headers[col_idx];
                            let type_tag = data
                                .col_types
                                .get(col_idx)
                                .map(|t| t.short_name())
                                .unwrap_or("???");
                            let sort_arrow = if vs.sort_col == Some(col_idx) {
                                if vs.sort_desc {
                                    " ▼"
                                } else {
                                    " ▲"
                                }
                            } else {
                                ""
                            };
                            let label_text = format!("{header} <{type_tag}>{sort_arrow}");
                            let resp = ui.add(
                                egui::Label::new(egui::RichText::new(label_text).strong())
                                    .sense(egui::Sense::click()),
                            );
                            if resp.clicked() {
                                if vs.sort_col == Some(col_idx) {
                                    vs.sort_desc = !vs.sort_desc;
                                } else {
                                    vs.sort_col = Some(col_idx);
                                    vs.sort_desc = false;
                                }
                                vs.selected_col = Some(col_idx);
                                vs.dirty = true;
                                vs.recompute(data);
                            }
                        }
                        ui.end_row();

                        // Data rows
                        for (vis_idx, &row_idx) in vs.visible_rows.iter().enumerate() {
                            let is_selected = vs.selected_row == Some(vis_idx);

                            // Row name
                            if let Some(rn) = data.row_names.get(row_idx) {
                                let text = egui::RichText::new(rn).weak();
                                let resp =
                                    ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                if resp.clicked() {
                                    vs.selected_row = Some(vis_idx);
                                }
                            }

                            // Cells
                            if let Some(row) = data.rows.get(row_idx) {
                                for &col_idx in &vis_cols {
                                    let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                                    let is_na = cell == "NA";
                                    let is_numeric =
                                        data.col_types.get(col_idx).is_some_and(|t| t.is_numeric());

                                    // Format the display value
                                    let display = if is_na {
                                        "NA".to_string()
                                    } else if let Some(digits) = vs.digits {
                                        if let Ok(v) = cell.parse::<f64>() {
                                            format!("{v:.digits$}")
                                        } else {
                                            cell.to_string()
                                        }
                                    } else {
                                        cell.to_string()
                                    };

                                    // Style: NA=gray italic, selected=highlight, numeric=monospace
                                    let mut text = if is_na {
                                        egui::RichText::new(&display).weak().italics()
                                    } else if is_selected {
                                        egui::RichText::new(&display)
                                            .background_color(egui::Color32::from_rgb(60, 80, 120))
                                    } else if is_numeric {
                                        egui::RichText::new(&display).monospace()
                                    } else {
                                        egui::RichText::new(&display)
                                    };

                                    // Search highlighting
                                    if !vs.filter.is_empty()
                                        && display
                                            .to_lowercase()
                                            .contains(&vs.filter.to_lowercase())
                                    {
                                        text = text.background_color(egui::Color32::from_rgb(
                                            120, 100, 30,
                                        ));
                                    }

                                    let layout = if is_numeric {
                                        egui::Layout::right_to_left(egui::Align::Center)
                                    } else {
                                        egui::Layout::left_to_right(egui::Align::Center)
                                    };
                                    ui.with_layout(layout, |ui| {
                                        let resp = ui.add(
                                            egui::Label::new(text).sense(egui::Sense::click()),
                                        );
                                        if resp.clicked() {
                                            vs.selected_row = Some(vis_idx);
                                            vs.selected_col = Some(col_idx);
                                        }
                                        resp.context_menu(|ui| {
                                            if ui.button("Copy value").clicked() {
                                                ui.ctx().copy_text(display.clone());
                                                ui.close();
                                            }
                                            if ui.button("Copy row").clicked() {
                                                if let Some(r) = data.rows.get(row_idx) {
                                                    ui.ctx().copy_text(r.join("\t"));
                                                }
                                                ui.close();
                                            }
                                            if ui.button("Copy column").clicked() {
                                                let col_vals: String = vs
                                                    .visible_rows
                                                    .iter()
                                                    .filter_map(|&ri| {
                                                        data.rows.get(ri)?.get(col_idx).cloned()
                                                    })
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                ui.ctx().copy_text(col_vals);
                                                ui.close();
                                            }
                                        });
                                    });
                                }
                            }
                            ui.end_row();
                        }
                    });
            });

        // Summary stats bar at bottom
        if let Some((min, max, mean, n)) = summary {
            ui.separator();
            let col_name = vs
                .selected_col
                .and_then(|c| data.headers.get(c))
                .map(|s| s.as_str())
                .unwrap_or("?");
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{col_name}: n={n}  min={min:.4}  mean={mean:.4}  max={max:.4}"
                    ))
                    .monospace()
                    .weak(),
                );
                if ui
                    .small_button("📊")
                    .on_hover_text("Column statistics")
                    .clicked()
                {
                    vs.show_stats_window = true;
                }
            });
        }
    });

    // Floating Column Statistics window
    if vs.show_stats_window {
        if let Some(col) = vs.selected_col {
            let col_name = data.headers.get(col).map(|s| s.as_str()).unwrap_or("?");
            let vals: Vec<f64> = vs
                .visible_rows
                .iter()
                .filter_map(|&r| data.rows.get(r)?.get(col)?.parse::<f64>().ok())
                .collect();

            egui::Window::new(format!("Statistics: {col_name}"))
                .open(&mut vs.show_stats_window)
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    if vals.is_empty() {
                        ui.label("No numeric values in this column.");
                        return;
                    }
                    let n = vals.len();
                    let na_count = vs.visible_rows.len() - n;
                    let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let sum: f64 = vals.iter().sum();
                    let mean = sum / n as f64;
                    let var = if n > 1 {
                        vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
                    } else {
                        0.0
                    };
                    let sd = var.sqrt();

                    let mut sorted = vals.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let q = |p: f64| -> f64 {
                        let idx = p * (sorted.len() - 1) as f64;
                        let lo = idx.floor() as usize;
                        let hi = idx.ceil() as usize;
                        let frac = idx - lo as f64;
                        sorted[lo] * (1.0 - frac) + sorted[hi.min(sorted.len() - 1)] * frac
                    };

                    egui::Grid::new("stats_grid")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            let row = |ui: &mut egui::Ui, label: &str, val: String| {
                                ui.label(label);
                                ui.label(egui::RichText::new(val).monospace());
                                ui.end_row();
                            };
                            row(ui, "n", format!("{n}"));
                            row(ui, "NA", format!("{na_count}"));
                            row(ui, "Min", format!("{min:.6}"));
                            row(ui, "Q1", format!("{:.6}", q(0.25)));
                            row(ui, "Median", format!("{:.6}", q(0.5)));
                            row(ui, "Q3", format!("{:.6}", q(0.75)));
                            row(ui, "Max", format!("{max:.6}"));
                            row(ui, "Mean", format!("{mean:.6}"));
                            row(ui, "SD", format!("{sd:.6}"));
                        });
                });
        }
    }
}

// endregion

// region: run_plot_event_loop

/// Run the plot event loop on the main thread.
///
/// Blocks on the channel until the first plot arrives, THEN launches the
/// egui window. This avoids creating a window (or dock icon on macOS)
/// when no plots are ever made. After the window is closed by the user
/// and all tabs are gone, eframe returns and we block again waiting for
/// the next plot.
pub fn run_plot_event_loop(rx: PlotReceiver) -> Result<(), String> {
    use std::sync::{Arc, Mutex};

    // Block on the main thread until the first plot/view arrives.
    // No GUI, no dock icon, no window — just waiting on the channel.
    let first_tab = loop {
        match rx.recv() {
            Ok(PlotMessage::Show(state)) => {
                let title = state.title.clone().unwrap_or_else(|| "Plot 1".to_string());
                break Tab::Plot {
                    title,
                    state,
                    view_state: PlotViewState::default(),
                };
            }
            Ok(PlotMessage::View(data)) => {
                let title = data.title.clone();
                let nrow = data.rows.len();
                break Tab::Table {
                    title,
                    view_state: TableViewState::new(data.headers.len(), nrow),
                    data,
                };
            }
            Ok(PlotMessage::Close) => continue,
            Err(_) => return Ok(()), // REPL exited without ever plotting
        }
    };

    // NOW launch the GUI — first plot/view is ready, window starts visible.
    let shared_rx = Arc::new(Mutex::new(rx));

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("miniR"),
        // Hide from dock on macOS — the plot window is an accessory, not the main app.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        })),
        ..Default::default()
    };

    let mut dark_mode = true;
    let first_tab_vec = vec![first_tab];
    let shared_rx_clone = Arc::clone(&shared_rx);

    // run_native blocks forever. eframe persistence saves/restores window geometry.
    eframe::run_native(
        "miniR",
        native_options,
        Box::new(move |cc| {
            // Load persisted preferences
            if let Some(storage) = cc.storage {
                if let Some(dm) = eframe::get_value::<bool>(storage, "dark_mode") {
                    dark_mode = dm;
                }
            }
            if !dark_mode {
                cc.egui_ctx.set_visuals(egui::Visuals::light());
            }
            Ok(Box::new(PlotApp {
                tabs: first_tab_vec,
                active_tab: 0,
                rx: shared_rx_clone,
                dark_mode,
                save_msg: None,
            }))
        }),
    )
    .map_err(|e| format!("egui event loop failed: {e}"))
}

// endregion

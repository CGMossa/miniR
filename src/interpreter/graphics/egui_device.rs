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
    /// Column range start (inclusive) for the range selector.
    col_range_start: usize,
    /// Column range end (inclusive) for the range selector.
    col_range_end: usize,
    /// Search filter for column names in the picker.
    col_picker_filter: String,
    /// Show column visibility panel.
    show_col_picker: bool,
    /// Show floating statistics window.
    show_stats_window: bool,
    /// Pre-parsed numeric values: numeric_cache[col][row] = Some(f64) or None.
    /// Populated once at construction time — avoids repeated parse::<f64>() per frame.
    numeric_cache: Vec<Vec<Option<f64>>>,
}

impl TableViewState {
    fn new(data: &TableData) -> Self {
        let ncol = data.headers.len();
        let nrow = data.rows.len();

        // Pre-parse all numeric values once
        let numeric_cache: Vec<Vec<Option<f64>>> = (0..ncol)
            .map(|col| {
                (0..nrow)
                    .map(|row| {
                        data.rows
                            .get(row)
                            .and_then(|r| r.get(col))
                            .and_then(|s| s.parse::<f64>().ok())
                    })
                    .collect()
            })
            .collect();

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
            col_range_start: 0,
            col_range_end: ncol.saturating_sub(1),
            col_picker_filter: String::new(),
            show_col_picker: false,
            show_stats_window: false,
            numeric_cache,
        }
    }

    /// Get the pre-parsed numeric value for a cell, or None.
    fn numeric_val(&self, col: usize, row: usize) -> Option<f64> {
        self.numeric_cache
            .get(col)
            .and_then(|c| c.get(row).copied().flatten())
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
            let cache = &self.numeric_cache;
            indices.sort_by(|&a, &b| {
                // Use pre-parsed numeric cache instead of parsing per comparison
                let cmp = match (
                    cache.get(col).and_then(|c| c.get(a).copied().flatten()),
                    cache.get(col).and_then(|c| c.get(b).copied().flatten()),
                ) {
                    (Some(fa), Some(fb)) => {
                        fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (Some(_), None) => std::cmp::Ordering::Less, // numbers before non-numbers
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => {
                        let va = data.rows[a].get(col).map(|s| s.as_str()).unwrap_or("");
                        let vb = data.rows[b].get(col).map(|s| s.as_str()).unwrap_or("");
                        va.cmp(vb)
                    }
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

    /// Apply the column range selector: set col_visible based on range.
    fn apply_col_range(&mut self) {
        for (i, vis) in self.col_visible.iter_mut().enumerate() {
            *vis = i >= self.col_range_start && i <= self.col_range_end;
        }
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
    /// Screenshot requested — will capture next frame as PNG.
    screenshot_requested: bool,
    screenshot_path: Option<std::path::PathBuf>,
    /// Windowed mode: each tab is a floating sub-window instead of tabs.
    windowed_mode: bool,
}

impl eframe::App for PlotApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dark_mode", &self.dark_mode);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle screenshot capture from previous frame
        if let Some(path) = self.screenshot_path.take() {
            for event in &ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    if let Err(e) = save_color_image_as_png(image, &path) {
                        self.save_msg =
                            Some((format!("PNG error: {e}"), std::time::Instant::now()));
                    } else {
                        self.save_msg = Some((
                            format!("Saved: {}", path.display()),
                            std::time::Instant::now(),
                        ));
                    }
                }
            }
        }

        // Send screenshot command if requested (captured next frame)
        if self.screenshot_requested {
            self.screenshot_requested = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }

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
        let rx = self.rx.lock().expect("plot channel mutex poisoned");
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
                    self.tabs.push(Tab::Table {
                        title,
                        view_state: TableViewState::new(&data),
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
        // Use a short interval even when hidden — macOS may throttle hidden windows.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

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

                // Right-aligned buttons: theme toggle + Open CSV
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

                    // Windowed mode toggle
                    let wm_icon = if self.windowed_mode { "▣" } else { "◫" };
                    if ui
                        .button(wm_icon)
                        .on_hover_text(if self.windowed_mode {
                            "Switch to tab mode"
                        } else {
                            "Switch to windowed mode"
                        })
                        .clicked()
                    {
                        self.windowed_mode = !self.windowed_mode;
                    }

                    #[cfg(feature = "io")]
                    if ui
                        .button("Open CSV")
                        .on_hover_text("Open a CSV/TSV file in a new View tab")
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv", "tsv", "tab", "txt"])
                            .pick_file()
                        {
                            match super::csv_drop::csv_to_table_data(&path) {
                                Ok(data) => {
                                    let title = data.title.clone();
                                    self.tabs.push(Tab::Table {
                                        title,
                                        view_state: TableViewState::new(&data),
                                        data,
                                    });
                                    self.active_tab = self.tabs.len() - 1;
                                }
                                Err(e) => {
                                    self.save_msg = Some((
                                        format!("CSV error: {e}"),
                                        std::time::Instant::now(),
                                    ));
                                }
                            }
                        }
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

        if self.windowed_mode {
            // Windowed mode: each tab is a floating sub-window
            let mut to_close = Vec::new();
            for (i, tab) in self.tabs.iter_mut().enumerate() {
                let title = tab.title().to_string();
                let mut open = true;
                match tab {
                    Tab::Plot {
                        state, view_state, ..
                    } => {
                        egui::Window::new(&title)
                            .id(egui::Id::new(format!("win_{i}")))
                            .open(&mut open)
                            .resizable(true)
                            .default_size([600.0, 400.0])
                            .show(ctx, |ui| {
                                let mut plot = egui_plot::Plot::new(format!("plot_{i}"))
                                    .show_grid(view_state.show_grid)
                                    .allow_boxed_zoom(true);
                                if view_state.show_legend {
                                    plot = plot.legend(egui_plot::Legend::default());
                                }
                                let ps = view_state.point_size;
                                let lw = view_state.line_width;
                                plot.show(ui, |plot_ui| {
                                    for (idx, item) in state.items.iter().enumerate() {
                                        let name = format!("series_{idx}");
                                        render_plot_item(plot_ui, item, &name, idx, ps, lw);
                                    }
                                });
                            });
                    }
                    Tab::Table {
                        data, view_state, ..
                    } => {
                        egui::Window::new(&title)
                            .id(egui::Id::new(format!("win_{i}")))
                            .open(&mut open)
                            .resizable(true)
                            .default_size([700.0, 400.0])
                            .show(ctx, |ui| {
                                render_table(ctx, ui, data, view_state);
                            });
                    }
                }
                if !open {
                    to_close.push(i);
                }
            }
            for &i in to_close.iter().rev() {
                self.tabs.remove(i);
            }
            if self.tabs.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        } else {
            // Tab mode: render active tab in central panel
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
                    egui::CentralPanel::default().show(ctx, |ui| {
                        render_table(ctx, ui, data, view_state);
                    });
                }
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
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("Rplot.svg")
                    .save_file()
                {
                    let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                    match std::fs::write(&path, svg_str) {
                        Ok(()) => {
                            *save_msg = Some((
                                format!("Saved: {}", path.display()),
                                std::time::Instant::now(),
                            ))
                        }
                        Err(e) => {
                            *save_msg = Some((format!("Error: {e}"), std::time::Instant::now()))
                        }
                    }
                }
            }

            #[cfg(feature = "pdf-device")]
            if ui.button("Save PDF").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("Rplot.pdf")
                    .save_file()
                {
                    let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                    match super::pdf::svg_to_pdf(&svg_str, 672.0, 672.0) {
                        Ok(bytes) => match std::fs::write(&path, bytes) {
                            Ok(()) => {
                                *save_msg = Some((
                                    format!("Saved: {}", path.display()),
                                    std::time::Instant::now(),
                                ))
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
            if ui.button("Save SVG...").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("Rplot.svg")
                    .save_file()
                {
                    let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                    match std::fs::write(&path, svg_str) {
                        Ok(()) => {
                            *save_msg = Some((
                                format!("Saved: {}", path.display()),
                                std::time::Instant::now(),
                            ))
                        }
                        Err(e) => {
                            *save_msg = Some((format!("Error: {e}"), std::time::Instant::now()))
                        }
                    }
                }
            }

            #[cfg(feature = "pdf-device")]
            if ui.button("Save PDF...").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("Rplot.pdf")
                    .save_file()
                {
                    let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                    match super::pdf::svg_to_pdf(&svg_str, 672.0, 672.0) {
                        Ok(bytes) => match std::fs::write(&path, bytes) {
                            Ok(()) => {
                                *save_msg = Some((
                                    format!("Saved: {}", path.display()),
                                    std::time::Instant::now(),
                                ))
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
            }

            #[cfg(all(feature = "svg-device", feature = "pdf-device"))]
            if ui.button("Save PNG...").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG", &["png"])
                    .set_file_name("Rplot.png")
                    .save_file()
                {
                    let svg_str = super::svg_device::render_svg(state, 7.0, 7.0);
                    match svg_to_png_bytes(&svg_str) {
                        Ok(bytes) => match std::fs::write(&path, bytes) {
                            Ok(()) => {
                                *save_msg = Some((
                                    format!("Saved: {}", path.display()),
                                    std::time::Instant::now(),
                                ))
                            }
                            Err(e) => {
                                *save_msg = Some((format!("Error: {e}"), std::time::Instant::now()))
                            }
                        },
                        Err(e) => {
                            *save_msg = Some((format!("PNG error: {e}"), std::time::Instant::now()))
                        }
                    }
                }
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

/// Render the View() toolbar, table grid, and summary bar into the given `Ui`.
///
/// The table uses `egui_table::Table` for virtual scrolling when the `view`
/// feature is enabled, falling back to `egui::Grid` (all rows) otherwise.
/// The caller is responsible for providing the `Ui` — this works inside both
/// `egui::CentralPanel` and `egui::Window`.
fn render_table(ctx: &egui::Context, ui: &mut egui::Ui, data: &TableData, vs: &mut TableViewState) {
    if vs.dirty {
        vs.recompute(data);
    }

    // Summary stats for selected column (uses pre-parsed numeric cache)
    let summary = vs.selected_col.and_then(|col| {
        let vals: Vec<f64> = vs
            .visible_rows
            .iter()
            .filter_map(|&r| vs.numeric_val(col, r))
            .collect();
        if vals.is_empty() {
            return None;
        }
        let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        Some((min, max, mean, vals.len()))
    });

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

    // Column visibility picker with range selector
    if vs.show_col_picker {
        let ncol = data.headers.len();

        // Range selector row
        ui.horizontal(|ui| {
            ui.label("Range:");
            let mut start = vs.col_range_start;
            let mut end = vs.col_range_end;
            ui.add(
                egui::DragValue::new(&mut start)
                    .range(0..=ncol.saturating_sub(1))
                    .prefix("from "),
            );
            ui.add(
                egui::DragValue::new(&mut end)
                    .range(0..=ncol.saturating_sub(1))
                    .prefix("to "),
            );
            if start != vs.col_range_start || end != vs.col_range_end {
                vs.col_range_start = start.min(ncol.saturating_sub(1));
                vs.col_range_end = end.min(ncol.saturating_sub(1));
                vs.apply_col_range();
            }

            ui.separator();
            if ui.small_button("All").clicked() {
                vs.col_visible.fill(true);
                vs.col_range_start = 0;
                vs.col_range_end = ncol.saturating_sub(1);
            }
            if ui.small_button("None").clicked() {
                vs.col_visible.fill(false);
            }

            ui.separator();
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut vs.col_picker_filter)
                    .desired_width(120.0)
                    .hint_text("column name..."),
            );
        });

        // Scrollable checkbox area with search filtering
        let picker_filter = vs.col_picker_filter.to_lowercase();
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .id_salt("col_picker_scroll")
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, header) in data.headers.iter().enumerate() {
                        if !picker_filter.is_empty()
                            && !header.to_lowercase().contains(&picker_filter)
                        {
                            continue;
                        }
                        let mut visible = vs.col_visible.get(i).copied().unwrap_or(true);
                        if ui.checkbox(&mut visible, header).changed() {
                            if let Some(v) = vs.col_visible.get_mut(i) {
                                *v = visible;
                            }
                        }
                    }
                });
            });
    }

    ui.separator();

    // Visible column indices
    let vis_cols: Vec<usize> = (0..data.headers.len())
        .filter(|&i| vs.col_visible.get(i).copied().unwrap_or(true))
        .collect();

    // Render the table body — virtual scrolling via egui_table when available,
    // otherwise fall back to egui::Grid (all rows).
    #[cfg(feature = "view")]
    {
        render_table_virtual(ui, data, vs, &vis_cols);
    }
    #[cfg(not(feature = "view"))]
    {
        render_table_grid(ui, data, vs, &vis_cols);
    }

    // Keyboard navigation
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
        let max_row = vs.visible_rows.len().saturating_sub(1);
        vs.selected_row = Some(vs.selected_row.map(|r| (r + 1).min(max_row)).unwrap_or(0));
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
        vs.selected_row = Some(vs.selected_row.map(|r| r.saturating_sub(1)).unwrap_or(0));
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
        let max_col = data.headers.len().saturating_sub(1);
        vs.selected_col = Some(vs.selected_col.map(|c| (c + 1).min(max_col)).unwrap_or(0));
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
        vs.selected_col = Some(vs.selected_col.map(|c| c.saturating_sub(1)).unwrap_or(0));
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
        if ctx.input(|i| i.modifiers.command) {
            vs.selected_col = Some(0);
        } else {
            vs.selected_row = Some(0);
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::End)) {
        if ctx.input(|i| i.modifiers.command) {
            vs.selected_col = Some(data.headers.len().saturating_sub(1));
        } else {
            vs.selected_row = Some(vs.visible_rows.len().saturating_sub(1));
        }
    }
    // Escape clears selection
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        vs.selected_row = None;
        vs.selected_col = None;
    }

    // Status bar at bottom
    ui.separator();
    ui.horizontal(|ui| {
        // Dimensions
        ui.label(
            egui::RichText::new(format!("{} x {}", data.rows.len(), data.headers.len()))
                .monospace()
                .weak(),
        );

        // Selection info
        if let Some(row) = vs.selected_row {
            ui.separator();
            let row_label = vs
                .visible_rows
                .get(row)
                .and_then(|&r| data.row_names.get(r))
                .map(|s| s.as_str())
                .unwrap_or("?");
            if let Some(col) = vs.selected_col {
                let col_name = data.headers.get(col).map(|s| s.as_str()).unwrap_or("?");
                ui.label(
                    egui::RichText::new(format!("Row {row_label}, Col '{col_name}'"))
                        .monospace()
                        .weak(),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("Row {row_label}"))
                        .monospace()
                        .weak(),
                );
            }
        }

        // Summary stats for selected numeric column
        if let Some((min, max, mean, n)) = summary {
            ui.separator();
            let col_name = vs
                .selected_col
                .and_then(|c| data.headers.get(c))
                .map(|s| s.as_str())
                .unwrap_or("?");
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
        }
    });

    // Floating Column Statistics window
    if vs.show_stats_window {
        if let Some(col) = vs.selected_col {
            let col_name = data.headers.get(col).map(|s| s.as_str()).unwrap_or("?");
            let vals: Vec<f64> = vs
                .visible_rows
                .iter()
                .filter_map(|&r| vs.numeric_val(col, r))
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

// region: egui_table virtual scrolling

/// Render the table body using `egui_table::Table` for virtual scrolling.
///
/// Only cells in the visible viewport are rendered — O(viewport) cost instead
/// of O(total_rows * total_cols). Used when the `view` feature is enabled.
#[cfg(feature = "view")]
fn render_table_virtual(
    ui: &mut egui::Ui,
    data: &TableData,
    vs: &mut TableViewState,
    vis_cols: &[usize],
) {
    // Build columns: col 0 = row names (sticky), rest = data cols
    let mut columns: Vec<egui_table::Column> = Vec::with_capacity(vis_cols.len() + 1);
    columns.push(
        egui_table::Column::new(80.0)
            .range(egui::Rangef::new(40.0, 300.0))
            .resizable(true),
    );
    for _ in vis_cols {
        columns.push(
            egui_table::Column::new(120.0)
                .range(egui::Rangef::new(40.0, 600.0))
                .resizable(true),
        );
    }

    let mut action = ViewTableAction::None;
    let row_count = u64::try_from(vs.visible_rows.len()).unwrap_or(0);
    {
        let mut delegate = ViewTableDelegate {
            data,
            vs,
            vis_cols,
            action: &mut action,
        };

        egui_table::Table::new()
            .id_salt("view_table")
            .columns(columns)
            .num_sticky_cols(1)
            .headers([egui_table::HeaderRow::new(24.0)])
            .num_rows(row_count)
            .auto_size_mode(egui_table::AutoSizeMode::OnParentResize)
            .show(ui, &mut delegate);
    }

    // Apply deferred actions from delegate clicks
    match action {
        ViewTableAction::None => {}
        ViewTableAction::SortColumn(col_idx) => {
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
        ViewTableAction::SelectRow(vis_idx) => {
            vs.selected_row = Some(vis_idx);
        }
        ViewTableAction::SelectCell(vis_idx, col_idx) => {
            vs.selected_row = Some(vis_idx);
            vs.selected_col = Some(col_idx);
        }
    }
}

/// Deferred action from a click inside the virtual table.
///
/// Because `egui_table::TableDelegate` methods receive `&mut self`, and we
/// need the borrow of `TableViewState` to remain shared during rendering, we
/// collect click intents here and apply them after `table.show()` returns.
#[cfg(feature = "view")]
enum ViewTableAction {
    None,
    SortColumn(usize),
    SelectRow(usize),
    SelectCell(usize, usize),
}

/// Delegate that feeds `egui_table::Table` with View() data.
///
/// Only cells in the visible viewport are rendered, giving O(viewport) cost
/// instead of O(total_rows * total_cols).
#[cfg(feature = "view")]
struct ViewTableDelegate<'a> {
    data: &'a TableData,
    vs: &'a TableViewState,
    /// Indices into `data.headers`/`data.rows[r]` for visible columns.
    vis_cols: &'a [usize],
    /// Collects the first click action per frame.
    action: &'a mut ViewTableAction,
}

#[cfg(feature = "view")]
impl egui_table::TableDelegate for ViewTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        let col_nr = cell.col_range.start;

        if col_nr == 0 {
            // Row-name header (sticky column)
            ui.label(egui::RichText::new("#").weak().monospace().size(11.0));
            return;
        }

        // Map table column (1-based) to data column index
        let Some(&data_col) = self.vis_cols.get(col_nr - 1) else {
            return;
        };

        // Selected column highlight
        if self.vs.selected_col == Some(data_col) {
            let tint = if ui.visuals().dark_mode {
                egui::Color32::from_rgba_premultiplied(60, 100, 180, 20)
            } else {
                egui::Color32::from_rgba_premultiplied(40, 80, 160, 15)
            };
            ui.painter().rect_filled(ui.max_rect(), 0.0, tint);
        }

        let header = &self.data.headers[data_col];
        let col_type = self.data.col_types.get(data_col).copied();
        let type_tag = col_type.map(|t| t.short_name()).unwrap_or("???");

        // Colored type badge
        let badge_color = match col_type {
            Some(super::view::ColType::Double) => egui::Color32::from_rgb(80, 140, 220),
            Some(super::view::ColType::Integer) => egui::Color32::from_rgb(200, 140, 50),
            Some(super::view::ColType::Character) => egui::Color32::from_rgb(80, 170, 100),
            Some(super::view::ColType::Logical) => egui::Color32::from_rgb(160, 100, 200),
            _ => egui::Color32::GRAY,
        };

        let sort_arrow = if self.vs.sort_col == Some(data_col) {
            if self.vs.sort_desc {
                " \u{25bc}"
            } else {
                " \u{25b2}"
            }
        } else {
            ""
        };

        // Build rich header: name + colored badge + sort arrow
        let resp = ui
            .horizontal(|ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(header).strong())
                        .sense(egui::Sense::click()),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(type_tag)
                            .size(10.0)
                            .color(badge_color)
                            .monospace(),
                    )
                    .sense(egui::Sense::click()),
                );
                if !sort_arrow.is_empty() {
                    ui.label(egui::RichText::new(sort_arrow).strong());
                }
            })
            .response;

        // Tooltip with column details
        let na_count = self
            .vs
            .numeric_cache
            .get(data_col)
            .map(|c| c.iter().filter(|v| v.is_none()).count())
            .unwrap_or(0);
        let resp = resp.on_hover_text(format!(
            "{header}\nType: {type_tag}\nNA: {na_count}\nColumn: {}",
            data_col + 1
        ));

        if resp.clicked() {
            *self.action = ViewTableAction::SortColumn(data_col);
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let vis_idx = cell.row_nr as usize;
        let col_nr = cell.col_nr;

        // Look up the original row index
        let Some(&row_idx) = self.vs.visible_rows.get(vis_idx) else {
            return;
        };

        // Zebra striping — subtle alternating row background
        if vis_idx.is_multiple_of(2) {
            let stripe = if ui.visuals().dark_mode {
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 6)
            } else {
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 8)
            };
            ui.painter().rect_filled(ui.max_rect(), 0.0, stripe);
        }

        // Column highlight — subtle tint for the selected column
        if col_nr > 0 {
            if let Some(&data_col_check) = self.vis_cols.get(col_nr - 1) {
                if self.vs.selected_col == Some(data_col_check) {
                    let col_tint = if ui.visuals().dark_mode {
                        egui::Color32::from_rgba_premultiplied(60, 100, 180, 15)
                    } else {
                        egui::Color32::from_rgba_premultiplied(40, 80, 160, 12)
                    };
                    ui.painter().rect_filled(ui.max_rect(), 0.0, col_tint);
                }
            }
        }

        if col_nr == 0 {
            // Row name (sticky column)
            if let Some(rn) = self.data.row_names.get(row_idx) {
                let text = egui::RichText::new(rn).weak().monospace().size(11.0);
                let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                if resp.clicked() {
                    *self.action = ViewTableAction::SelectRow(vis_idx);
                }
            }
            return;
        }

        // Map table column to data column
        let Some(&data_col) = self.vis_cols.get(col_nr - 1) else {
            return;
        };

        let is_row_selected = self.vs.selected_row == Some(vis_idx);
        let is_col_selected = self.vs.selected_col == Some(data_col);
        let is_cell_selected = is_row_selected && is_col_selected;

        let Some(row) = self.data.rows.get(row_idx) else {
            return;
        };
        let cell_val = row.get(data_col).map(|s| s.as_str()).unwrap_or("");
        let is_na = cell_val == "NA";
        let is_numeric = self
            .data
            .col_types
            .get(data_col)
            .is_some_and(|t| t.is_numeric());

        // Format the display value — use numeric cache for digits formatting
        let display = if is_na {
            "NA".to_string()
        } else if let Some(digits) = self.vs.digits {
            if let Some(v) = self.vs.numeric_val(data_col, row_idx) {
                format!("{v:.digits$}")
            } else {
                cell_val.to_string()
            }
        } else {
            cell_val.to_string()
        };

        // Style: cell selection > row selection > NA > numeric > default
        let mut text = if is_cell_selected {
            egui::RichText::new(&display)
                .background_color(egui::Color32::from_rgb(50, 90, 150))
                .color(egui::Color32::WHITE)
        } else if is_row_selected {
            egui::RichText::new(&display).background_color(egui::Color32::from_rgb(60, 80, 120))
        } else if is_na {
            egui::RichText::new(&display).weak().italics()
        } else if is_numeric {
            egui::RichText::new(&display).monospace()
        } else {
            egui::RichText::new(&display)
        };

        // Search highlighting
        if !self.vs.filter.is_empty()
            && display
                .to_lowercase()
                .contains(&self.vs.filter.to_lowercase())
        {
            text = text.background_color(egui::Color32::from_rgb(120, 100, 30));
        }

        // Right-align numeric columns
        if is_numeric {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                if resp.clicked() {
                    *self.action = ViewTableAction::SelectCell(vis_idx, data_col);
                }
                Self::cell_context_menu(
                    &resp,
                    &display,
                    self.data,
                    row_idx,
                    data_col,
                    &self.vs.visible_rows,
                );
            });
        } else {
            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
            if resp.clicked() {
                *self.action = ViewTableAction::SelectCell(vis_idx, data_col);
            }
            Self::cell_context_menu(
                &resp,
                &display,
                self.data,
                row_idx,
                data_col,
                &self.vs.visible_rows,
            );
        }
    }

    fn default_row_height(&self) -> f32 {
        20.0
    }
}

#[cfg(feature = "view")]
impl ViewTableDelegate<'_> {
    /// Attach a right-click context menu to a cell response.
    fn cell_context_menu(
        resp: &egui::Response,
        display: &str,
        data: &TableData,
        row_idx: usize,
        col_idx: usize,
        visible_rows: &[usize],
    ) {
        let display_owned = display.to_string();
        resp.context_menu(|ui| {
            if ui.button("Copy value").clicked() {
                ui.ctx().copy_text(display_owned.clone());
                ui.close();
            }
            if ui.button("Copy row").clicked() {
                if let Some(r) = data.rows.get(row_idx) {
                    ui.ctx().copy_text(r.join("\t"));
                }
                ui.close();
            }
            if ui.button("Copy column").clicked() {
                let col_vals: String = visible_rows
                    .iter()
                    .filter_map(|&ri| data.rows.get(ri)?.get(col_idx).cloned())
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.ctx().copy_text(col_vals);
                ui.close();
            }
        });
    }
}

// endregion

// region: egui::Grid fallback (no virtual scrolling)

/// Fallback table body renderer using `egui::Grid` + `egui::ScrollArea`.
///
/// Creates a widget for every cell in every row — fine for small tables, but
/// catastrophic for large data frames. Used when the `view` feature is disabled.
#[cfg(not(feature = "view"))]
fn render_table_grid(
    ui: &mut egui::Ui,
    data: &TableData,
    vs: &mut TableViewState,
    vis_cols: &[usize],
) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("view_grid")
                .striped(true)
                .num_columns(vis_cols.len() + 1)
                .min_col_width(40.0)
                .show(ui, |ui| {
                    // Header row
                    ui.label(egui::RichText::new("").weak());
                    for &col_idx in vis_cols {
                        let header = &data.headers[col_idx];
                        let type_tag = data
                            .col_types
                            .get(col_idx)
                            .map(|t| t.short_name())
                            .unwrap_or("???");
                        let sort_arrow = if vs.sort_col == Some(col_idx) {
                            if vs.sort_desc {
                                " \u{25bc}"
                            } else {
                                " \u{25b2}"
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

                        if let Some(rn) = data.row_names.get(row_idx) {
                            let text = egui::RichText::new(rn).weak();
                            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if resp.clicked() {
                                vs.selected_row = Some(vis_idx);
                            }
                        }

                        if let Some(row) = data.rows.get(row_idx) {
                            for &col_idx in vis_cols {
                                let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                                let is_na = cell == "NA";
                                let is_numeric =
                                    data.col_types.get(col_idx).is_some_and(|t| t.is_numeric());

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

                                if !vs.filter.is_empty()
                                    && display.to_lowercase().contains(&vs.filter.to_lowercase())
                                {
                                    text = text
                                        .background_color(egui::Color32::from_rgb(120, 100, 30));
                                }

                                let layout = if is_numeric {
                                    egui::Layout::right_to_left(egui::Align::Center)
                                } else {
                                    egui::Layout::left_to_right(egui::Align::Center)
                                };
                                ui.with_layout(layout, |ui| {
                                    let resp =
                                        ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.clicked() {
                                        vs.selected_row = Some(vis_idx);
                                        vs.selected_col = Some(col_idx);
                                    }
                                });
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

// endregion

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
                break Tab::Table {
                    title,
                    view_state: TableViewState::new(&data),
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
                screenshot_requested: false,
                screenshot_path: None,
                windowed_mode: false,
            }))
        }),
    )
    .map_err(|e| format!("egui event loop failed: {e}"))
}

// endregion

// region: PNG export helpers

/// Convert an SVG string to PNG bytes via resvg rasterization.
#[cfg(all(feature = "svg-device", feature = "pdf-device"))]
fn svg_to_png_bytes(svg_str: &str) -> Result<Vec<u8>, String> {
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_str, &opts).map_err(|e| format!("SVG parse error: {e}"))?;
    let size = tree.size();
    let w = size.width() as u32;
    let h = size.height() as u32;
    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).ok_or_else(|| "failed to create pixmap".to_string())?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let img = image::RgbaImage::from_raw(w, h, pixmap.take())
        .ok_or_else(|| "pixel buffer mismatch".to_string())?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {e}"))?;
    Ok(buf.into_inner())
}

/// Save an egui ColorImage as PNG (for screenshots).
#[allow(dead_code)]
fn save_color_image_as_png(
    color_image: &egui::ColorImage,
    path: &std::path::Path,
) -> Result<(), String> {
    let [w, h] = color_image.size;
    let w_u32 = u32::try_from(w).map_err(|e| format!("width overflow: {e}"))?;
    let h_u32 = u32::try_from(h).map_err(|e| format!("height overflow: {e}"))?;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for pixel in &color_image.pixels {
        let [r, g, b, a] = pixel.to_array();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    let img = image::RgbaImage::from_raw(w_u32, h_u32, rgba)
        .ok_or_else(|| "pixel buffer mismatch".to_string())?;
    img.save(path).map_err(|e| format!("PNG save error: {e}"))
}

// endregion

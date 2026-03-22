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

/// A single tab in the GUI window.
enum Tab {
    Plot {
        title: String,
        state: PlotState,
    },
    Table {
        title: String,
        data: TableData,
        view_state: TableViewState,
    },
}

/// Interactive state for a View() table tab.
struct TableViewState {
    /// Text filter — only rows containing this substring (in any column) are shown.
    filter: String,
    /// Column to sort by (None = original order).
    sort_col: Option<usize>,
    /// Sort descending?
    sort_desc: bool,
    /// Number of decimal digits for numeric display (None = raw).
    digits: Option<usize>,
    /// Cached sorted+filtered row indices.
    visible_rows: Vec<usize>,
    /// Whether visible_rows needs recomputation.
    dirty: bool,
}

impl TableViewState {
    fn new(nrow: usize) -> Self {
        Self {
            filter: String::new(),
            sort_col: None,
            sort_desc: false,
            digits: None,
            visible_rows: (0..nrow).collect(),
            dirty: false,
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
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // X button: let the window close normally. The outer loop in
        // run_plot_event_loop will block until the next plot arrives.

        // Check for messages from the REPL thread (non-blocking).
        let rx = self.rx.lock().unwrap();
        while let Ok(msg) = rx.try_recv() {
            match msg {
                PlotMessage::Show(new_state) => {
                    let title = new_state
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("Plot {}", self.tabs.len() + 1));
                    self.tabs.push(Tab::Plot {
                        title,
                        state: new_state,
                    });
                    self.active_tab = self.tabs.len() - 1;
                }
                PlotMessage::View(data) => {
                    let title = data.title.clone();
                    let nrow = data.rows.len();
                    self.tabs.push(Tab::Table {
                        title,
                        view_state: TableViewState::new(nrow),
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
                        // Close window — outer loop will block until next plot
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    }
                }
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
                    if ui.small_button("×").clicked() {
                        to_close = Some(i);
                    }
                    ui.separator();
                }
                if let Some(idx) = to_close {
                    self.tabs.remove(idx);
                    if self.tabs.is_empty() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
        match &mut self.tabs[active] {
            Tab::Plot { state, .. } => {
                render_plot(ctx, state);
            }
            Tab::Table {
                data, view_state, ..
            } => {
                render_table(ctx, data, view_state);
            }
        }
    }
}

/// Render a plot in the central panel.
fn render_plot(ctx: &egui::Context, state: &PlotState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let title = state.title.as_deref().unwrap_or("Plot");

        let mut plot = egui_plot::Plot::new("r_plot")
            .legend(egui_plot::Legend::default())
            .show_axes(true)
            .show_grid(true);

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

        ui.heading(title);

        plot.show(ui, |plot_ui| {
            for (idx, item) in state.items.iter().enumerate() {
                let default_name = format!("series_{idx}");
                render_plot_item(plot_ui, item, &default_name, idx);
            }
        });
    });
}

fn render_plot_item(
    plot_ui: &mut egui_plot::PlotUi,
    item: &PlotItem,
    default_name: &str,
    idx: usize,
) {
    match item {
        PlotItem::Line {
            x,
            y,
            color,
            width,
            label,
        } => {
            let points: Vec<[f64; 2]> = x.iter().zip(y.iter()).map(|(&xi, &yi)| [xi, yi]).collect();
            let name = label.as_deref().unwrap_or(default_name);
            plot_ui.line(
                egui_plot::Line::new(name, points)
                    .color(rgba_to_color32(*color))
                    .width(*width),
            );
        }
        PlotItem::Points {
            x,
            y,
            color,
            size,
            shape,
            label,
        } => {
            let points: Vec<[f64; 2]> = x.iter().zip(y.iter()).map(|(&xi, &yi)| [xi, yi]).collect();
            let name = label.as_deref().unwrap_or(default_name);
            plot_ui.points(
                egui_plot::Points::new(name, points)
                    .color(rgba_to_color32(*color))
                    .radius(*size)
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
        PlotItem::HLine { y, color, width } => {
            plot_ui.hline(
                egui_plot::HLine::new(format!("hline_{idx}"), *y)
                    .color(rgba_to_color32(*color))
                    .width(*width),
            );
        }
        PlotItem::VLine { x, color, width } => {
            plot_ui.vline(
                egui_plot::VLine::new(format!("vline_{idx}"), *x)
                    .color(rgba_to_color32(*color))
                    .width(*width),
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
    // Recompute visible rows if dirty
    if vs.dirty {
        vs.recompute(data);
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading(&data.title);

        // Toolbar: filter, digits control, row count
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let old_filter = vs.filter.clone();
            ui.text_edit_singleline(&mut vs.filter);
            if vs.filter != old_filter {
                vs.dirty = true;
                vs.recompute(data);
            }

            ui.separator();
            ui.label("Digits:");
            let mut digits_str = vs
                .digits
                .map(|d| d.to_string())
                .unwrap_or_else(|| "auto".to_string());
            if ui.text_edit_singleline(&mut digits_str).changed() {
                vs.digits = digits_str.parse::<usize>().ok();
            }

            ui.separator();
            ui.label(format!(
                "{}/{} rows",
                vs.visible_rows.len(),
                data.rows.len()
            ));
        });
        ui.separator();

        let ncol = data.headers.len();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("view_grid")
                    .striped(true)
                    .num_columns(ncol + 1)
                    .min_col_width(40.0)
                    .show(ui, |ui| {
                        // Header row — clickable for sorting
                        ui.label(egui::RichText::new("").weak());
                        for (col_idx, header) in data.headers.iter().enumerate() {
                            let label = if vs.sort_col == Some(col_idx) {
                                let arrow = if vs.sort_desc { " ▼" } else { " ▲" };
                                format!("{header}{arrow}")
                            } else {
                                header.clone()
                            };
                            if ui
                                .add(
                                    egui::Label::new(egui::RichText::new(label).strong())
                                        .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                if vs.sort_col == Some(col_idx) {
                                    vs.sort_desc = !vs.sort_desc;
                                } else {
                                    vs.sort_col = Some(col_idx);
                                    vs.sort_desc = false;
                                }
                                vs.dirty = true;
                                vs.recompute(data);
                            }
                        }
                        ui.end_row();

                        // Data rows (filtered + sorted)
                        for &row_idx in &vs.visible_rows {
                            // Row name
                            if let Some(rn) = data.row_names.get(row_idx) {
                                ui.label(egui::RichText::new(rn).weak());
                            }
                            // Cells
                            if let Some(row) = data.rows.get(row_idx) {
                                for cell in row {
                                    if cell == "NA" {
                                        ui.label(egui::RichText::new("NA").weak().italics());
                                    } else {
                                        // Apply digit formatting for numbers
                                        let display = if let Some(digits) = vs.digits {
                                            if let Ok(v) = cell.parse::<f64>() {
                                                format!("{v:.digits$}")
                                            } else {
                                                cell.clone()
                                            }
                                        } else {
                                            cell.clone()
                                        };
                                        ui.label(&display);
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    });
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
    // We need to share the receiver between the outer blocking loop and
    // the eframe app. Use a shared wrapper that lets us take the first
    // message out before passing the receiver to eframe.
    use std::sync::{Arc, Mutex};

    let shared_rx = Arc::new(Mutex::new(rx));

    loop {
        // Block until a Show or View message arrives — no window exists.
        let first_tab = {
            let rx = shared_rx.lock().unwrap();
            loop {
                match rx.recv() {
                    Ok(PlotMessage::Show(state)) => {
                        let title = state.title.clone().unwrap_or_else(|| "Plot 1".to_string());
                        break Tab::Plot { title, state };
                    }
                    Ok(PlotMessage::View(data)) => {
                        let title = data.title.clone();
                        let nrow = data.rows.len();
                        break Tab::Table {
                            title,
                            view_state: TableViewState::new(nrow),
                            data,
                        };
                    }
                    Ok(PlotMessage::Close) => continue,
                    Err(_) => return Ok(()), // REPL exited
                }
            }
        };

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800.0, 600.0])
                .with_title("miniR"),
            ..Default::default()
        };

        let app = PlotApp {
            tabs: vec![first_tab],
            active_tab: 0,
            rx: Arc::clone(&shared_rx),
        };

        // run_native blocks until the window is closed.
        let _ = eframe::run_native("miniR", native_options, Box::new(|_cc| Ok(Box::new(app))));
        // Window closed — loop back and wait for the next plot/view.
    }
}

// endregion

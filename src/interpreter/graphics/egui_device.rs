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

// region: PlotChannel

/// Message from the REPL thread to the GUI thread.
pub enum PlotMessage {
    /// Show a new plot (replaces the current one).
    Show(PlotState),
    /// Close the current plot window.
    Close,
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
    Plot { title: String, state: PlotState },
}

impl Tab {
    fn title(&self) -> &str {
        match self {
            Tab::Plot { title, .. } => title,
        }
    }
}

/// The eframe app. Manages tabbed plots from the REPL thread.
/// Starts hidden; becomes visible when the first plot arrives.
/// Hides again when all tabs are closed (ready for the next plot).
struct PlotApp {
    tabs: Vec<Tab>,
    active_tab: usize,
    rx: PlotReceiver,
    visible: bool,
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Intercept window close (X button): hide instead of quitting,
        // so the window can reappear for the next plot.
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.tabs.clear();
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        // Check for messages from the REPL thread (non-blocking).
        while let Ok(msg) = self.rx.try_recv() {
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
                    // Show the window when the first plot arrives
                    if !self.visible {
                        self.visible = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    }
                }
                PlotMessage::Close => {
                    if !self.tabs.is_empty() {
                        self.tabs.remove(self.active_tab);
                        if self.active_tab > 0 {
                            self.active_tab -= 1;
                        }
                    }
                    if self.tabs.is_empty() {
                        // Hide instead of closing — ready for the next plot
                        self.visible = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    }
                }
            }
        }

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
                    if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
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
        match &self.tabs[active] {
            Tab::Plot { state, .. } => {
                render_plot(ctx, state);
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

// endregion

// region: run_plot_event_loop

/// Run the egui event loop on the main thread.
///
/// Manages a tabbed window for plots and View() tables. The REPL sends
/// messages through the channel; each plot/view gets its own tab.
pub fn run_plot_event_loop(rx: PlotReceiver) -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("miniR")
            .with_visible(false), // Start hidden; show when first plot arrives
        ..Default::default()
    };

    let app = PlotApp {
        tabs: Vec::new(),
        active_tab: 0,
        rx,
        visible: false,
    };

    eframe::run_native("miniR", native_options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| format!("egui event loop failed: {e}"))
}

// endregion

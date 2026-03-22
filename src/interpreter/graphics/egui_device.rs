//! Interactive plot window using egui/eframe/egui_plot.
//!
//! This module is only compiled when the `plot` feature is enabled.
//! It converts `PlotState` into an egui_plot window that supports
//! pan, zoom, hover, and legend display.
//!
//! The window runs on a separate thread so the REPL remains interactive.
//! `dev.off()` sends a close signal, and the window's X button also works.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use super::plot_data::{PlotItem, PlotState};

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

/// Whether a pch value represents a filled marker.
fn pch_is_filled(pch: u8) -> bool {
    pch >= 15
}

/// Convert an `[u8; 4]` RGBA color to an egui `Color32`.
fn rgba_to_color32(c: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Handle to a running plot window. Dropping it or calling `close()` shuts
/// the window down.
pub struct PlotWindowHandle {
    close_tx: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PlotWindowHandle {
    /// Request the window to close and wait for the thread to finish.
    pub fn close(&mut self) {
        let _ = self.close_tx.send(());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PlotWindowHandle {
    fn drop(&mut self) {
        self.close();
    }
}

/// The eframe app that displays a single plot.
struct PlotApp {
    state: PlotState,
    close_rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if we've been asked to close
        if let Ok(rx) = self.close_rx.lock() {
            if rx.try_recv().is_ok() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let title = self.state.title.as_deref().unwrap_or("Plot");

            let mut plot = egui_plot::Plot::new("r_plot")
                .legend(egui_plot::Legend::default())
                .show_axes(true)
                .show_grid(true);

            if let Some(label) = &self.state.x_label {
                plot = plot.x_axis_label(label.clone());
            }
            if let Some(label) = &self.state.y_label {
                plot = plot.y_axis_label(label.clone());
            }

            if let Some((lo, hi)) = self.state.x_lim {
                plot = plot.include_x(lo).include_x(hi);
            }
            if let Some((lo, hi)) = self.state.y_lim {
                plot = plot.include_y(lo).include_y(hi);
            }

            // Show title above the plot
            ui.heading(title);

            plot.show(ui, |plot_ui| {
                for (idx, item) in self.state.items.iter().enumerate() {
                    let default_name = format!("series_{idx}");
                    render_plot_item(plot_ui, item, &default_name, idx);
                }
            });
        });
    }
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
            let line = egui_plot::Line::new(name, points)
                .color(rgba_to_color32(*color))
                .width(*width);
            plot_ui.line(line);
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
            let pts = egui_plot::Points::new(name, points)
                .color(rgba_to_color32(*color))
                .radius(*size)
                .shape(pch_to_marker(*shape))
                .filled(pch_is_filled(*shape));
            plot_ui.points(pts);
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
            let chart = egui_plot::BarChart::new(name, bars);
            plot_ui.bar_chart(chart);
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
                let box_name = format!("box_{j}");
                plot_ui.box_plot(egui_plot::BoxPlot::new(box_name, vec![elem]));
            }
        }
        PlotItem::HLine { y, color, width } => {
            let name = format!("hline_{idx}");
            let hline = egui_plot::HLine::new(name, *y)
                .color(rgba_to_color32(*color))
                .width(*width);
            plot_ui.hline(hline);
        }
        PlotItem::VLine { x, color, width } => {
            let name = format!("vline_{idx}");
            let vline = egui_plot::VLine::new(name, *x)
                .color(rgba_to_color32(*color))
                .width(*width);
            plot_ui.vline(vline);
        }
        PlotItem::Text { x, y, text, color } => {
            let name = format!("text_{idx}");
            let txt = egui_plot::Text::new(
                name,
                egui_plot::PlotPoint::new(*x, *y),
                egui::RichText::new(text).color(rgba_to_color32(*color)),
            );
            plot_ui.text(txt);
        }
    }
}

/// Launch a non-blocking egui window displaying the plot.
///
/// Returns a `PlotWindowHandle` that can be used to close the window
/// programmatically (via `dev.off()`). The window also closes when
/// the user clicks the X button.
pub fn show_plot_window(state: &PlotState) -> Result<PlotWindowHandle, String> {
    let (close_tx, close_rx) = mpsc::channel();
    let close_rx = Arc::new(Mutex::new(close_rx));
    let owned_state = state.clone();
    let title = state.title.as_deref().unwrap_or("R Plot").to_string();

    let thread = std::thread::Builder::new()
        .name("plot-window".into())
        .spawn(move || {
            let native_options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([800.0, 600.0])
                    .with_title(&title),
                ..Default::default()
            };

            let app = PlotApp {
                state: owned_state,
                close_rx,
            };

            // run_native blocks until the window is closed
            let _ = eframe::run_native(&title, native_options, Box::new(|_cc| Ok(Box::new(app))));
        })
        .map_err(|e| format!("failed to spawn plot thread: {e}"))?;

    Ok(PlotWindowHandle {
        close_tx,
        thread: Some(thread),
    })
}

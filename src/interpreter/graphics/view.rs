//! View() data frame viewer using egui_table.
//!
//! Sends table data to the GUI thread for rendering in a scrollable
//! spreadsheet-like window with sticky row names, resizable columns,
//! and virtual scrolling for large data frames.

// region: TableData

/// Column type for display formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColType {
    Double,
    Integer,
    Character,
    Logical,
    Other,
}

impl ColType {
    pub fn short_name(self) -> &'static str {
        match self {
            ColType::Double => "dbl",
            ColType::Integer => "int",
            ColType::Character => "chr",
            ColType::Logical => "lgl",
            ColType::Other => "???",
        }
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, ColType::Double | ColType::Integer)
    }
}

/// Pre-formatted table data for display.
#[derive(Debug, Clone)]
pub struct TableData {
    pub title: String,
    pub headers: Vec<String>,
    pub col_types: Vec<ColType>,
    pub row_names: Vec<String>,
    /// rows[row][col] — pre-formatted cell strings.
    pub rows: Vec<Vec<String>>,
}

// endregion

// region: ViewMessage

/// Message from the REPL thread to the GUI thread for View().
#[derive(Debug, Clone)]
pub enum ViewMessage {
    /// Show a data frame viewer.
    Show(TableData),
}

// endregion

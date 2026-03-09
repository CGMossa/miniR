# egui_table View Backend

Implement R's `View()` function using `egui_table` + `eframe` to display data frames in a native scrollable spreadsheet-like window.

## Dependencies

```toml
# Only compiled when the "view" feature is enabled — keeps default builds fast
[features]
default = []
view = ["dep:eframe", "dep:egui", "dep:egui_table"]

[dependencies]
eframe = { version = "0.33", optional = true }
egui = { version = "0.33", optional = true }
egui_table = { version = "0.7", optional = true }
```

The `view` feature is opt-in. `View()` without the feature prints a warning and falls back to `print()`.

**Note:** Requires Rust 1.88+ (edition 2024) due to egui_table's MSRV.

## What View() does in R

```r
View(df)           # Opens spreadsheet viewer for data frame
View(df, "My DF")  # With custom title
View(mtcars)       # Works on any data frame
```

- Opens a **read-only** spreadsheet window
- Columns have headers (from `names(df)`)
- Rows have row numbers (or `row.names(df)`)
- Scrollable, resizable columns
- Window title defaults to the expression passed (deparse of the argument)
- Non-blocking in RStudio, blocking in base R — we'll start blocking

## Architecture

### View builtin

`View()` is a pre-eval builtin (needs the unevaluated expression for the window title):

```rust
#[pre_eval_builtin(name = "View", min_args = 1)]
fn pre_eval_view(args: &[Expr], env: &Environment) -> Result<RValue, RError> {
    let value = eval(args[0], env)?;
    let title = args.get(1)
        .map(|e| eval_to_string(e, env))
        .unwrap_or_else(|| deparse_expr(&args[0]));

    view::show_view(value, title)?;
    Ok(RValue::invisible_null())
}
```

### View module (`src/interpreter/view.rs`)

```rust
#[cfg(feature = "view")]
pub fn show_view(value: RValue, title: String) -> Result<(), RError> {
    // Extract columns + headers from data frame or matrix
    let table_data = extract_table_data(&value)?;

    // Launch eframe window (blocking)
    eframe::run_native(
        &title,
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(ViewApp::new(table_data)))),
    ).map_err(|e| RError::Other(format!("View error: {}", e)))
}

#[cfg(not(feature = "view"))]
pub fn show_view(value: RValue, _title: String) -> Result<(), RError> {
    eprintln!("View() requires the 'view' feature. Falling back to print.");
    println!("{}", value);
    Ok(())
}
```

### ViewApp (egui application via TableDelegate)

egui_table uses a **delegate pattern** — you implement the `TableDelegate` trait to supply cell contents. This supports millions of rows via virtual scrolling (only visible rows are rendered).

```rust
struct ViewApp {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,    // Pre-formatted cell strings
    row_names: Vec<String>,
    columns: Vec<egui_table::Column>,
}

impl ViewApp {
    fn new(data: TableData) -> Self {
        // +1 column for row names
        let mut columns = vec![egui_table::Column::auto().resizable(true)];
        for _ in &data.headers {
            columns.push(egui_table::Column::auto().resizable(true));
        }
        ViewApp {
            headers: data.headers,
            rows: data.rows,
            row_names: data.row_names,
            columns,
        }
    }
}

impl egui_table::TableDelegate for ViewApp {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        if cell.col_nr == 0 {
            ui.label("");  // row name column header
        } else {
            ui.strong(&self.headers[cell.col_nr - 1]);
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let row = cell.row_nr as usize;
        if cell.col_nr == 0 {
            ui.label(&self.row_names[row]);
        } else {
            ui.label(&self.rows[row][cell.col_nr - 1]);
        }
    }

    fn default_row_height(&self) -> f32 {
        20.0
    }
}

impl eframe::App for ViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let num_rows = self.rows.len() as u64;
            egui_table::Table::new()
                .num_rows(num_rows)
                .columns(self.columns.clone())
                .num_sticky_cols(1)  // Freeze row-name column
                .headers(vec![egui_table::HeaderRow::new(self.default_row_height())])
                .show(ui, self);
        });
    }
}
```

### Key API notes (egui_table 0.7)

- `Table::new()` → builder with `.num_rows()`, `.columns()`, `.num_sticky_cols()`, `.headers()`
- `TableDelegate` trait — **required**: `header_cell_ui()`, `cell_ui()`; **optional**: `prepare()` (prefetch visible range), `row_ui()` (per-row styling), `default_row_height()`, `row_top_offset()` (heterogeneous heights)
- `CellInfo` has `col_nr: usize`, `row_nr: u64`, `table_id: Id`
- `Column::auto().resizable(true)` for auto-sized resizable columns
- Virtual scrolling is built in — handles millions of rows efficiently
- `num_sticky_cols(1)` freezes the row-name column during horizontal scroll

### Data extraction

`extract_table_data()` handles:

| Input type | Extraction |
|-----------|-----------|
| Data frame (list-based) | Each list element is a column; names from `names` attr |
| Data frame (polars) | Iterate polars columns, format each |
| Matrix (RVector with dim) | Reshape into rows × cols |
| Vector | Single-column table |
| List | Each element as a row |

Each cell is pre-formatted to `String` for display — keeps the egui render loop simple.

### Column formatting

| R type | Format |
|--------|--------|
| `double` | `format!("{:.6}", v)` trimmed trailing zeros |
| `integer` | `format!("{}", v)` |
| `character` | As-is (quoted in R, unquoted in View) |
| `logical` | `TRUE` / `FALSE` |
| `NA` | `NA` (grayed out in UI) |
| `NULL` | `NULL` |
| `factor` | Display label, not underlying integer |

## Features

### MVP (first implementation)

- Open a window with column headers and scrollable rows
- Row numbers on the left (sticky column)
- Window title from expression or argument
- Close window to return to REPL
- Works for data frames and matrices
- Resizable columns (auto-sized initially)

### Future enhancements

- Column sorting (click header to sort)
- Search/filter bar
- Column type indicators (int, chr, dbl, lgl) in headers
- Non-blocking (spawn window in separate thread, return to REPL immediately)
- Copy cell/row/column to clipboard
- Export visible data (filtered) to CSV
- Alternating row colors via `row_ui()` override
- NA cells styled differently (gray italic)
- Numeric columns right-aligned
- `prepare()` delegate for lazy loading large data frames
- Hierarchical column headers for grouped data

## Implementation order

1. Add `eframe`, `egui`, `egui_table` as optional deps behind `view` feature
2. Create `src/interpreter/view.rs` with feature-gated `show_view()`
3. Implement `ViewApp` with `TableDelegate` trait
4. Wire `View()` as a pre-eval builtin
5. Add `extract_table_data()` for data frames, matrices, vectors
6. Test with `View(mtcars)`, `View(iris)` etc.
7. Add column sorting
8. Add non-blocking mode (spawn thread)

## Build considerations

- `eframe` pulls in significant dependencies (winit, wgpu/glow, etc.) — ~200 crates
- Default builds should NOT include the `view` feature
- CI should test both `cargo build` and `cargo build --features view`
- Consider a `justfile` recipe: `just build-view` → `cargo build --features view`

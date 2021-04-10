//! A table of widgets with static number of columns.
//!
//! Use by implementing `TableRow` and adding instances of that type to a `Table` using `rows_mut`.
use base::basic_types::*;
use base::{StyleModifier, Window};
use input::Scrollable;
use input::{Behavior, Input, Navigatable, OperationResult};
use std::cell::Cell;
use widget::{
    layout_linearly, ColDemand, Demand, Demand2D, RenderingHints, RowDemand, SeparatingStyle,
    Widget,
};

/// A single column in a `Table`.
///
/// This does not store any data, but rather how to access a cell in a single column of a table
/// and how it reacts to input.
///
/// In a sense this is only necessary because we do not have variadic generics.
pub struct Column<T: TableRow + ?Sized> {
    /// Immutable widget access.
    pub access: for<'a> fn(&'a T) -> Box<dyn Widget + 'a>,
    /// Input processing
    pub behavior: fn(&mut T, Input, &mut T::BehaviorContext) -> Option<Input>,
}

/// This trait both (statically) describes the layout of the table (`COLUMNS`) and represents a
/// single row in the table.
///
/// Implement this trait, if you want to create a `Table`!
pub trait TableRow: 'static {
    type BehaviorContext;
    /// Define the behavior of individual columns of the table.
    const COLUMNS: &'static [Column<Self>];

    /// Convenient access using `COLUMNS`. (Do not reimplement this.)
    fn num_columns() -> usize {
        Self::COLUMNS.len()
    }

    /// Calculate the vertical space demand of the current row. (Default: max of all cells.)
    fn height_demand(&self) -> RowDemand {
        let mut y_demand = Demand::zero();
        for col in Self::COLUMNS.iter() {
            let demand2d = (col.access)(self).space_demand();
            y_demand.max_assign(demand2d.height);
        }
        y_demand
    }
}

/// Mutable row access mapper to enforce invariants after mutation.
pub struct RowsMut<'a, R: 'static + TableRow> {
    table: &'a mut Table<R>,
}

impl<'a, R: 'static + TableRow> ::std::ops::Drop for RowsMut<'a, R> {
    fn drop(&mut self) {
        let _ = self.table.validate_row_pos();
    }
}

impl<'a, R: 'static + TableRow> ::std::ops::Deref for RowsMut<'a, R> {
    type Target = Vec<R>;
    fn deref(&self) -> &Self::Target {
        &self.table.rows
    }
}

impl<'a, R: 'static + TableRow> ::std::ops::DerefMut for RowsMut<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table.rows
    }
}

/// A table of widgets with static number of `Columns`.
///
/// In order to create a table, you have to define a type for a row in the table and implement
/// `TableRow` for it. Then add instances of that type using `rows_mut`.
///
/// At any time, a single cell of the table is active. Send user input to the cell by adding the
/// result of `current_cell_behavior()` to an `InputChain`.
/// A table is also `Navigatable` by which the user can change which cell is the currently active
/// one.
pub struct Table<R: TableRow> {
    rows: Vec<R>,
    row_pos: u32,
    col_pos: u32,
    last_draw_pos: Cell<(u32, RowIndex)>,
}

impl<R: TableRow + 'static> Table<R> {
    /// Create an empty table and specify how rows/columns and the currently active cell will be
    /// distinguished.
    pub fn new() -> Self {
        Table {
            rows: Vec::new(),
            row_pos: 0,
            col_pos: 0,
            last_draw_pos: Cell::new((0, RowIndex::new(0))),
        }
    }

    /// Access the content of the table mutably.
    pub fn rows_mut<'a>(&'a mut self) -> RowsMut<'a, R> {
        RowsMut { table: self }
    }

    /// Access the content of the table immutably.
    pub fn rows(&mut self) -> &Vec<R> {
        &self.rows
    }

    fn validate_row_pos(&mut self) -> Result<(), ()> {
        let max_pos = (self.rows.len() as u32).checked_sub(1).unwrap_or(0);
        if self.row_pos > max_pos {
            self.row_pos = max_pos;
            Err(())
        } else {
            Ok(())
        }
    }

    fn validate_col_pos(&mut self) -> Result<(), ()> {
        let max_pos = R::num_columns() as u32 - 1;
        if self.col_pos > max_pos {
            self.col_pos = max_pos;
            Err(())
        } else {
            Ok(())
        }
    }

    /// Get access to the currently active row.
    pub fn current_row(&self) -> Option<&R> {
        self.rows.get(self.row_pos as usize)
    }

    /// Get mutable access to the currently active row.
    pub fn current_row_mut(&mut self) -> Option<&mut R> {
        self.rows.get_mut(self.row_pos as usize)
    }

    /// Get the currently active column.
    pub fn current_col(&self) -> &'static Column<R> {
        &R::COLUMNS[self.col_pos as usize]
    }

    fn pass_event_to_current_cell(
        &mut self,
        i: Input,
        p: &mut R::BehaviorContext,
    ) -> Option<Input> {
        let col_behavior = self.current_col().behavior;
        if let Some(row) = self.current_row_mut() {
            col_behavior(row, i, p)
        } else {
            Some(i)
        }
    }

    /// Create a `Behavior` which can be used to send input directly to the currently active cell
    /// by adding it to an `InputChain`.
    pub fn current_cell_behavior<'a, 'b>(
        &'a mut self,
        p: &'b mut R::BehaviorContext,
    ) -> CurrentCellBehavior<'a, 'b, R> {
        CurrentCellBehavior { table: self, p }
    }

    pub fn as_widget<'a>(&'a self) -> TableWidget<'a, R> {
        TableWidget {
            table: self,
            row_sep_style: SeparatingStyle::None,
            col_sep_style: SeparatingStyle::None,
            focused_style: StyleModifier::new(),
            min_context: 1,
        }
    }
}

/// Pass all behavior to the currently active cell.
pub struct CurrentCellBehavior<'a, 'b, R: TableRow + 'static> {
    table: &'a mut Table<R>,
    p: &'b mut R::BehaviorContext,
}

impl<R: TableRow + 'static> Behavior for CurrentCellBehavior<'_, '_, R> {
    fn input(self, i: Input) -> Option<Input> {
        self.table.pass_event_to_current_cell(i, self.p)
    }
}

pub struct TableWidget<'a, R: TableRow + 'static> {
    table: &'a Table<R>,
    row_sep_style: SeparatingStyle,
    col_sep_style: SeparatingStyle,
    focused_style: StyleModifier,
    min_context: u32,
}

impl<'a, R: TableRow + 'static> TableWidget<'a, R> {
    pub fn row_separation(mut self, style: SeparatingStyle) -> Self {
        self.row_sep_style = style;
        self
    }

    pub fn col_separation(mut self, style: SeparatingStyle) -> Self {
        self.col_sep_style = style;
        self
    }

    pub fn focused(mut self, style: StyleModifier) -> Self {
        self.focused_style = style;
        self
    }

    pub fn min_context(mut self, rows: u32) -> Self {
        self.min_context = rows;
        self
    }

    fn layout_columns(&self, window: &Window) -> Box<[Width]> {
        let mut x_demands = vec![Demand::zero(); R::num_columns()];
        for row in self.table.rows.iter() {
            for (col_num, col) in R::COLUMNS.iter().enumerate() {
                let demand2d = (col.access)(row).space_demand();
                x_demands[col_num].max_assign(demand2d.width);
            }
        }
        let separator_width = self.col_sep_style.width();
        let weights = std::iter::repeat(1.0)
            .take(x_demands.len())
            .collect::<Vec<f64>>();
        layout_linearly(window.get_width(), separator_width, &x_demands, &weights)
    }

    fn draw_row<'w>(
        &self,
        row: &R,
        row_index: u32,
        mut window: Window<'w>,
        column_widths: &[Width],
        hints: RenderingHints,
    ) {
        if let (1, &SeparatingStyle::AlternatingStyle(modifier)) =
            (row_index % 2, &self.row_sep_style)
        {
            window.modify_default_style(modifier);
        }

        let mut iter = R::COLUMNS
            .iter()
            .zip(column_widths.iter())
            .enumerate()
            .peekable();
        while let Some((col_index, (col, &pos))) = iter.next() {
            let (mut cell_window, r) = window
                .split(pos.from_origin())
                .expect("valid split pos from layout");
            window = r;

            if let (1, &SeparatingStyle::AlternatingStyle(modifier)) =
                (col_index % 2, &self.col_sep_style)
            {
                cell_window.modify_default_style(modifier);
            }

            let cell_draw_hints =
                if row_index == self.table.row_pos && col_index as u32 == self.table.col_pos {
                    cell_window.modify_default_style(self.focused_style);
                    hints
                } else {
                    hints.active(false)
                };

            cell_window.clear(); // Fill background using new style
            (col.access)(row).draw(cell_window, cell_draw_hints);
            if let (Some(_), &SeparatingStyle::Draw(ref c)) = (iter.peek(), &self.col_sep_style) {
                if window.get_width() > 0 {
                    let (mut sep_window, r) = window
                        .split(Width::from(c.width()).from_origin())
                        .expect("valid split pos from layout");
                    window = r;
                    sep_window.fill(c.clone());
                }
            }
        }
    }
    fn rows_space_demand(&self, rows: &[R]) -> Demand2D {
        let mut x_demands = vec![Demand::exact(0); R::num_columns()];
        let mut y_demand = Demand::zero();

        let mut row_iter = rows.iter().peekable();
        while let Some(row) = row_iter.next() {
            let mut row_max_y = Demand::exact(0);
            for (col_num, col) in R::COLUMNS.iter().enumerate() {
                let demand2d = (col.access)(row).space_demand();
                x_demands[col_num].max_assign(demand2d.width);
                row_max_y.max_assign(demand2d.height)
            }
            y_demand += row_max_y;
            if row_iter.peek().is_some() {
                y_demand += Demand::exact(self.row_sep_style.height());
            }
        }

        //Account all separators between cols
        let x_demand = x_demands.iter().sum::<ColDemand>()
            + ColDemand::exact(
                (self.col_sep_style.width() * (x_demands.len() as i32 - 1)).positive_or_zero(),
            );
        Demand2D {
            width: x_demand,
            height: y_demand,
        }
    }
}

impl<'a, R: TableRow + 'static> Widget for TableWidget<'a, R> {
    fn space_demand(&self) -> Demand2D {
        self.rows_space_demand(&self.table.rows[..])
    }
    fn draw(&self, window: Window, hints: RenderingHints) {
        fn split_top(window: Window, pos: RowIndex) -> (Window, Option<Window>) {
            match window.split(pos) {
                Ok((window, below)) => (window, Some(below)),
                Err(window) => (window, None),
            }
        }

        fn split_bottom(window: Window, pos: RowIndex) -> (Option<Window>, Window) {
            let split_pos = (window.get_height().from_origin() - pos).from_origin();
            match window.split(split_pos) {
                Ok((above, window)) => (Some(above), window),
                Err(window) => (None, window),
            }
        }

        let separator_height = if let &SeparatingStyle::Draw(_) = &self.row_sep_style {
            Height::new_unchecked(1)
        } else {
            Height::new_unchecked(0)
        };

        let max_height = window.get_height();
        let row_height = |r: &R| r.height_demand().max.unwrap_or(max_height); //TODO: choose min or max here and below?
        let demand_height = |d: Demand2D| d.height.max.unwrap_or(max_height);

        let column_widths = self.layout_columns(&window);

        let current = if let Some(r) = self.table.current_row() {
            r
        } else {
            return;
        };
        let current_row_height = row_height(current);
        let current_row_pos = self.table.row_pos;

        let (old_pos, old_draw_row) = self.table.last_draw_pos.get();
        let current_row_begin = match old_pos.cmp(&current_row_pos) {
            std::cmp::Ordering::Less => {
                let range = &self.table.rows[old_pos as usize..current_row_pos as usize];
                old_draw_row + demand_height(self.rows_space_demand(range)) + separator_height
            }
            std::cmp::Ordering::Equal => old_draw_row,
            std::cmp::Ordering::Greater => {
                let range = &self.table.rows
                    [current_row_pos as usize..(old_pos as usize).min(self.table.rows.len())];
                old_draw_row - demand_height(self.rows_space_demand(range)) - separator_height
            }
        };

        let min_diff = Height::new_unchecked(self.min_context as i32);

        let current_row_begin = current_row_begin
            .min(window.get_height().from_origin() - current_row_height - min_diff)
            .max(min_diff.from_origin());

        let widgets_above = &self.table.rows[..current_row_pos as usize];
        let widgets_below = &self.table.rows[(current_row_pos + 1) as usize..];

        let max_above_height = demand_height(self.rows_space_demand(widgets_above))
            + if widgets_above.is_empty() {
                Height::new_unchecked(0)
            } else {
                separator_height
            };
        let max_below_height = demand_height(self.rows_space_demand(widgets_below))
            + if widgets_below.is_empty() {
                Height::new_unchecked(0)
            } else {
                separator_height
            };

        let min_current_pos_begin =
            window.get_height().from_origin() - max_below_height - current_row_height;
        let max_current_pos_begin = max_above_height.from_origin();

        let current_row_begin = current_row_begin
            .max(min_current_pos_begin)
            .min(max_current_pos_begin);

        self.table
            .last_draw_pos
            .set((current_row_pos, current_row_begin));

        let (window, mut below) = split_top(window, current_row_begin + current_row_height);
        let (mut above, window) = split_bottom(window, current_row_height.from_origin());

        self.draw_row(current, current_row_pos, window, &column_widths, hints);

        // All rows below current
        for (row_pos, row) in widgets_below
            .iter()
            .enumerate()
            .map(|(i, row)| (i as u32 + current_row_pos + 1, row))
        {
            if let &SeparatingStyle::Draw(ref c) = &self.row_sep_style {
                if let Some(w) = below {
                    let (mut sep_window, rest) = split_top(w, RowIndex::from(1));
                    below = rest;

                    sep_window.fill(c.clone());
                } else {
                    break;
                }
            }

            if let Some(w) = below {
                let (row_window, rest) = split_top(w, row_height(row).from_origin());
                below = rest;
                self.draw_row(row, row_pos, row_window, &column_widths, hints);
            } else {
                break;
            }
        }

        // All rows above current
        for (row_pos, row) in widgets_above
            .iter()
            .enumerate()
            .rev()
            .map(|(i, row)| (i as u32, row))
        {
            if let &SeparatingStyle::Draw(ref c) = &self.row_sep_style {
                if let Some(w) = above {
                    let (rest, mut sep_window) = split_bottom(w, RowIndex::from(1));
                    above = rest;

                    sep_window.fill(c.clone());
                } else {
                    break;
                }
            }

            if let Some(w) = above {
                let (rest, row_window) = split_bottom(w, row_height(row).from_origin());
                above = rest;
                self.draw_row(row, row_pos, row_window, &column_widths, hints);
            } else {
                break;
            }
        }
    }
}

impl<R: TableRow + 'static> Navigatable for Table<R> {
    fn move_up(&mut self) -> OperationResult {
        if self.row_pos > 0 {
            self.row_pos -= 1;
            Ok(())
        } else {
            Err(())
        }
    }
    fn move_down(&mut self) -> OperationResult {
        self.row_pos += 1;
        self.validate_row_pos()
    }
    fn move_left(&mut self) -> OperationResult {
        if self.col_pos != 0 {
            self.col_pos -= 1;
            Ok(())
        } else {
            Err(())
        }
    }
    fn move_right(&mut self) -> OperationResult {
        self.col_pos += 1;
        self.validate_col_pos()
    }
}

impl<R: TableRow + 'static> Scrollable for Table<R> {
    fn scroll_backwards(&mut self) -> OperationResult {
        self.move_up()
    }
    fn scroll_forwards(&mut self) -> OperationResult {
        self.move_down()
    }
    fn scroll_to_beginning(&mut self) -> OperationResult {
        if self.row_pos != 0 {
            self.row_pos = 0;
            Ok(())
        } else {
            Err(())
        }
    }
    fn scroll_to_end(&mut self) -> OperationResult {
        let end = self.rows.len().saturating_sub(1) as u32;
        if self.row_pos != end {
            self.row_pos = end;
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use base::test::FakeTerminal;
    use base::{GraphemeCluster, StyleModifier};

    struct TestRow(String);
    impl TableRow for TestRow {
        type BehaviorContext = ();
        const COLUMNS: &'static [Column<Self>] = &[Column {
            access: |r| Box::new(r.0.as_str()),
            behavior: |_, _, _| None,
        }];
    }

    fn test_table(num_rows: usize) -> Table<TestRow> {
        let mut table = Table::new();
        {
            let mut rows = table.rows_mut();
            for i in 0..num_rows {
                rows.push(TestRow(i.to_string()));
            }
        }
        table
    }

    fn test_table_str(lines: &[&str]) -> Table<TestRow> {
        let mut table = Table::new();
        {
            let mut rows = table.rows_mut();
            for l in lines {
                rows.push(TestRow(l.to_string()));
            }
        }
        table
    }

    fn aeq_table_draw(
        terminal_size: (u32, u32),
        solution: &str,
        table: &Table<TestRow>,
        f: impl Fn(TableWidget<TestRow>) -> TableWidget<TestRow>,
    ) {
        let mut term = FakeTerminal::with_size(terminal_size);
        f(table.as_widget()).draw(term.create_root_window(), RenderingHints::default());
        assert_eq!(
            term,
            FakeTerminal::from_str(terminal_size, solution).expect("term from str"),
            "got <-> expected"
        );
    }
    fn aeq_table_draw_focused_bold(
        terminal_size: (u32, u32),
        solution: &str,
        table: &Table<TestRow>,
    ) {
        aeq_table_draw(terminal_size, solution, table, |t: TableWidget<TestRow>| {
            t.focused(StyleModifier::new().bold(true))
        });
    }

    fn aeq_table_draw_focused_bold_sep_x(
        terminal_size: (u32, u32),
        solution: &str,
        table: &Table<TestRow>,
    ) {
        aeq_table_draw(terminal_size, solution, table, |t: TableWidget<TestRow>| {
            t.focused(StyleModifier::new().bold(true))
                .row_separation(SeparatingStyle::Draw(
                    GraphemeCluster::try_from('X').unwrap(),
                ))
        });
    }

    #[test]
    fn smaller_than_terminal() {
        aeq_table_draw((1, 3), "0 1 2", &test_table(10), |t| t);
    }

    #[test]
    fn scroll_down_simple() {
        let mut table = test_table(6);
        let size = (1, 4);
        aeq_table_draw_focused_bold(size, "*0* 1 2 3", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "0 *1* 2 3", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "0 1 *2* 3", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "1 2 *3* 4", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "2 3 *4* 5", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "2 3 4 *5*", &table);
        assert!(table.move_down().is_err());
    }

    #[test]
    fn scroll_down_sep() {
        let mut table = test_table(4);
        let size = (1, 4);
        aeq_table_draw_focused_bold_sep_x(size, "*0* X 1 X", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "0 X *1* X", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "1 X *2* X", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "X 2 X *3*", &table);
        assert!(table.move_down().is_err());
    }

    #[test]
    fn scroll_down_multiline() {
        let mut table = test_table_str(&["a\nb", "c", "d\ne\n", "f", "g\nh"]);
        let size = (1, 4);
        aeq_table_draw_focused_bold(size, "*ab* c d", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "ab *c* d", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "c *de* f", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "de *f* g", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold(size, "d f *gh*", &table);
        assert!(table.move_down().is_err());
    }

    #[test]
    fn scroll_down_multiline_sep() {
        let mut table = test_table_str(&["a\nb", "c", "d\ne\nf", "f", "g\nh"]);
        let size = (1, 4);
        aeq_table_draw_focused_bold_sep_x(size, "*ab* X c", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "a X *c* X", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "X *def*", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "d X *f* X", &table);
        table.move_down().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "f X *gh*", &table);
        assert!(table.move_down().is_err());
    }

    #[test]
    fn scroll_up_simple() {
        let mut table = test_table(6);
        let size = (1, 4);
        table.scroll_to_end().unwrap();
        aeq_table_draw_focused_bold(size, "2 3 4 *5*", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold(size, "2 3 *4* 5", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold(size, "2 *3* 4 5", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold(size, "1 *2* 3 4", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold(size, "0 *1* 2 3", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold(size, "*0* 1 2 3", &table);
        assert!(table.move_up().is_err());
    }

    #[test]
    fn scroll_up_sep() {
        let mut table = test_table(4);
        let size = (1, 4);
        table.scroll_to_end().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "X 2 X *3*", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "X *2* X 3", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "X *1* X 2", &table);
        table.move_up().unwrap();
        aeq_table_draw_focused_bold_sep_x(size, "*0* X 1 X", &table);
        assert!(table.move_up().is_err());
    }

    #[test]
    fn sep_alternate_rows() {
        let table = test_table(4);
        aeq_table_draw((1, 4), "0 *1* 2 *3*", &table, |t| {
            t.row_separation(SeparatingStyle::AlternatingStyle(
                StyleModifier::new().bold(true),
            ))
        });
    }

    #[test]
    fn sep_char() {
        let table = test_table(4);
        aeq_table_draw((1, 7), "0 X 1 X 2 X 3", &table, |t| {
            t.row_separation(SeparatingStyle::Draw(
                GraphemeCluster::try_from('X').unwrap(),
            ))
        });
    }

    #[test]
    fn sep_none() {
        let table = test_table(4);
        aeq_table_draw((1, 4), "0 1 2 3", &table, |t| {
            t.row_separation(SeparatingStyle::None)
        });
    }
}

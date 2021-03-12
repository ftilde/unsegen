//! A table of widgets with static number of columns.
//!
//! Use by implementing `TableRow` and adding instances of that type to a `Table` using `rows_mut`.
use base::basic_types::*;
use base::{StyleModifier, Window};
use input::Scrollable;
use input::{Behavior, Input, Navigatable, OperationResult};
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
pub struct Column<T: ?Sized> {
    /// Immutable widget access.
    pub access: for<'a> fn(&'a T) -> Box<dyn Widget + 'a>,
    /// Input processing
    pub behavior: fn(&mut T, Input) -> Option<Input>,
}

/// This trait both (statically) describes the layout of the table (`COLUMNS`) and represents a
/// single row in the table.
///
/// Implement this trait, if you want to create a `Table`!
pub trait TableRow: 'static {
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
}

impl<R: TableRow + 'static> Table<R> {
    /// Create an empty table and specify how rows/columns and the currently active cell will be
    /// distinguished.
    pub fn new() -> Self {
        Table {
            rows: Vec::new(),
            row_pos: 0,
            col_pos: 0,
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

    fn pass_event_to_current_cell(&mut self, i: Input) -> Option<Input> {
        let col_behavior = self.current_col().behavior;
        if let Some(row) = self.current_row_mut() {
            col_behavior(row, i)
        } else {
            Some(i)
        }
    }

    /// Create a `Behavior` which can be used to send input directly to the currently active cell
    /// by adding it to an `InputChain`.
    pub fn current_cell_behavior<'a>(&'a mut self) -> CurrentCellBehavior<'a, R> {
        CurrentCellBehavior { table: self }
    }

    pub fn as_widget<'a>(&'a self) -> TableWidget<'a, R> {
        TableWidget {
            table: self,
            row_sep_style: SeparatingStyle::None,
            col_sep_style: SeparatingStyle::None,
            focused_style: StyleModifier::new(),
        }
    }
}

/// Pass all behavior to the currently active cell.
pub struct CurrentCellBehavior<'a, R: TableRow + 'static> {
    table: &'a mut Table<R>,
}

impl<'a, R: TableRow + 'static> Behavior for CurrentCellBehavior<'a, R> {
    fn input(self, i: Input) -> Option<Input> {
        self.table.pass_event_to_current_cell(i)
    }
}

pub struct TableWidget<'a, R: TableRow + 'static> {
    table: &'a Table<R>,
    row_sep_style: SeparatingStyle,
    col_sep_style: SeparatingStyle,
    focused_style: StyleModifier,
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

    fn layout_columns(&self, window: &Window) -> Box<[Width]> {
        let mut x_demands = vec![Demand::zero(); R::num_columns()];
        for row in self.table.rows.iter() {
            for (col_num, col) in R::COLUMNS.iter().enumerate() {
                let demand2d = (col.access)(row).space_demand();
                x_demands[col_num].max_assign(demand2d.width);
            }
        }
        let separator_width = self.col_sep_style.width();
        layout_linearly(window.get_width(), separator_width, &x_demands)
    }
}

impl<'a, R: TableRow + 'static> Widget for TableWidget<'a, R> {
    fn space_demand(&self) -> Demand2D {
        let mut x_demands = vec![Demand::exact(0); R::num_columns()];
        let mut y_demand = Demand::zero();

        let mut row_iter = self.table.rows.iter().peekable();
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
    fn draw(&self, window: Window, hints: RenderingHints) {
        let column_widths = self.layout_columns(&window);

        let mut window = Some(window);
        let mut row_iter = self.table.rows.iter().enumerate().peekable();
        while let Some((row_index, row)) = row_iter.next() {
            if window.is_none() {
                break;
            }
            let height = row.height_demand().min;
            let (mut row_window, rest_window) = match window.unwrap().split(height.from_origin()) {
                Ok((row_window, rest_window)) => (row_window, Some(rest_window)),
                Err(row_window) => (row_window, None),
            };
            window = rest_window;

            if let (1, &SeparatingStyle::AlternatingStyle(modifier)) =
                (row_index % 2, &self.row_sep_style)
            {
                row_window.modify_default_style(modifier);
            }

            let mut iter = R::COLUMNS
                .iter()
                .zip(column_widths.iter())
                .enumerate()
                .peekable();
            while let Some((col_index, (col, &pos))) = iter.next() {
                let (mut cell_window, r) = row_window
                    .split(pos.from_origin())
                    .expect("valid split pos from layout");
                row_window = r;

                if let (1, &SeparatingStyle::AlternatingStyle(modifier)) =
                    (col_index % 2, &self.col_sep_style)
                {
                    cell_window.modify_default_style(modifier);
                }

                let cell_draw_hints = if row_index as u32 == self.table.row_pos
                    && col_index as u32 == self.table.col_pos
                {
                    cell_window.modify_default_style(self.focused_style);
                    hints
                } else {
                    hints.active(false)
                };

                cell_window.clear(); // Fill background using new style
                (col.access)(row).draw(cell_window, cell_draw_hints);
                if let (Some(_), &SeparatingStyle::Draw(ref c)) = (iter.peek(), &self.col_sep_style)
                {
                    if row_window.get_width() > 0 {
                        let (mut sep_window, r) = row_window
                            .split(Width::from(c.width()).from_origin())
                            .expect("valid split pos from layout");
                        row_window = r;
                        sep_window.fill(c.clone());
                    }
                }
            }
            if let (Some(_), &SeparatingStyle::Draw(ref c)) = (row_iter.peek(), &self.row_sep_style)
            {
                if window.is_none() {
                    break;
                }
                let (mut sep_window, rest_window) =
                    match window.unwrap().split(height.from_origin()) {
                        Ok((row_window, rest_window)) => (row_window, Some(rest_window)),
                        Err(row_window) => (row_window, None),
                    };
                window = rest_window;
                sep_window.fill(c.clone());
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
    use base::StyleModifier;

    struct TestRow(String);
    impl TableRow for TestRow {
        const COLUMNS: &'static [Column<Self>] = &[Column {
            access: |r| Box::new(r.0.as_str()),
            behavior: |_, _| None,
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

    fn aeq_table_draw(terminal_size: (u32, u32), solution: &str, table: &Table<TestRow>) {
        let mut term = FakeTerminal::with_size(terminal_size);
        table
            .as_widget()
            .focused(StyleModifier::new().bold(true))
            .draw(term.create_root_window(), RenderingHints::default());
        assert_eq!(
            term,
            FakeTerminal::from_str(terminal_size, solution).expect("term from str")
        );
    }

    #[test]
    fn smaller_than_terminal() {
        aeq_table_draw((1, 3), "*0* 1 2", &test_table(10));
        //aeq_table_draw((1, 3), "0 1 ↓", &test_table(10));
    }

    #[test]
    fn scroll_down() {
        let mut table = test_table(6);
        let size = (1, 4);
        aeq_table_draw(size, "*0* 1 2 3", &table);
        //aeq_table_draw((4, 1), "0 1 2 ↓", &test_table(10));
        table.move_down().unwrap();
        aeq_table_draw(size, "0 *1* 2 3", &table);
        table.move_down().unwrap();
        aeq_table_draw(size, "0 1 *2* 3", &table);
        table.move_down().unwrap();
        aeq_table_draw(size, "0 1 2 *3*", &table);
        table.move_down().unwrap();
        aeq_table_draw(size, "0 1 2 3", &table);
        table.move_down().unwrap();
        aeq_table_draw(size, "0 1 2 3", &table);
        assert!(table.move_down().is_err());
    }
}

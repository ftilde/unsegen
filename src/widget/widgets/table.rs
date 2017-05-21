use base::{
    Window,
    StyleModifier,
};
use input::{
    Behavior,
    Input,
    Navigatable,
    OperationResult,
};
use widget::{
    Demand,
    SeparatingStyle,
    Widget,
    layout_linearly,
};

pub struct Column<T: ?Sized> {
    // This will be SO much more convenient to implement once this is stablized:
    // https://github.com/rust-lang/rust/issues/39817
    pub access: fn(&T) -> &Widget,
    pub access_mut: fn(&mut T) -> &mut Widget,
    pub behavior: fn(&mut T, Input) -> Option<Input>,
}

pub trait TableRow {
    fn columns() -> &'static [Column<Self>];

    fn num_columns() -> usize where Self: 'static {
        Self::columns().len()
    }

    fn height_demand(&self) -> Demand where Self: 'static {
        let mut y_demand = Demand::zero();
        for col in Self::columns().iter() {
            let (_, y) = (col.access)(self).space_demand();
            y_demand.max_assign(y);
        }
        y_demand
    }
}

pub struct RowsMut<'a, R: 'static + TableRow> {
    table: &'a mut Table<R>,
}

impl<'a, R: 'static + TableRow> ::std::ops::Drop for RowsMut<'a, R> {
    fn drop(&mut self) {
        let _ = self.table.validate_row_pos();
    }
}

impl<'a, R: 'static + TableRow> ::std::ops::DerefMut for RowsMut<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table.rows
    }
}

impl<'a, R: 'static + TableRow> ::std::ops::Deref for RowsMut<'a, R> {
    type Target = Vec<R>;
    fn deref(&self) -> &Self::Target {
        &self.table.rows
    }
}

pub struct Table<R: TableRow> {
    rows: Vec<R>,
    row_sep_style: SeparatingStyle,
    col_sep_style: SeparatingStyle,
    focused_style: StyleModifier,
    row_pos: u32,
    col_pos: u32,
}

impl<R: TableRow + 'static> Table<R> {
    pub fn new(row_sep_style: SeparatingStyle, col_sep_style: SeparatingStyle, focused_style: StyleModifier) -> Self {
        Table {
            rows: Vec::new(),
            row_sep_style: row_sep_style,
            col_sep_style: col_sep_style,
            focused_style: focused_style,
            row_pos: 0,
            col_pos: 0,
        }
    }

    pub fn rows_mut<'a>(&'a mut self) -> RowsMut<'a, R> {
        RowsMut {
            table: self
        }
    }

    pub fn rows(&mut self) -> &Vec<R> {
        &self.rows
    }

    fn layout_columns(&self, window: &Window) -> Box<[u32]> {
        let mut x_demands = vec![Demand::zero(); R::num_columns()];
        for row in self.rows.iter() {
            for (col_num, col) in R::columns().iter().enumerate() {
                let (x, _) = (col.access)(row).space_demand();
                x_demands[col_num].max_assign(x);
            }
        }
        let separator_width = self.col_sep_style.width();
        layout_linearly(window.get_width(), separator_width, &x_demands)
    }

    fn validate_row_pos(&mut self) -> Result<(),()> {
        let max_pos = (self.rows.len() as u32).checked_sub(1).unwrap_or(0);
        if self.row_pos > max_pos {
            self.row_pos = max_pos;
            Err(())
        } else {
            Ok(())
        }
    }

    fn validate_col_pos(&mut self) -> Result<(),()> {
        let max_pos = R::num_columns() as u32 - 1;
        if self.col_pos > max_pos {
            self.col_pos = max_pos;
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn current_row_mut(&mut self) -> Option<&mut R> {
        self.rows.get_mut(self.row_pos as usize)
    }

    pub fn current_col(&self) -> &'static Column<R> {
        &R::columns()[self.col_pos as usize]
    }

    fn pass_event_to_current_cell(&mut self, i: Input) -> Option<Input> {
        let col_behavior = self.current_col().behavior;
        if let Some(row) = self.current_row_mut() {
            col_behavior(row, i)
        } else {
            Some(i)
        }
    }

    pub fn current_cell_behavior<'a>(&'a mut self) -> CurrentCellBehavior<'a, R> {
        CurrentCellBehavior {
            table: self,
        }
    }
}

pub struct CurrentCellBehavior<'a, R: TableRow + 'static> {
    table: &'a mut Table<R>,
}

impl<'a, R: TableRow + 'static> Behavior for CurrentCellBehavior<'a, R> {
    fn input(self, i: Input) -> Option<Input> {
        self.table.pass_event_to_current_cell(i)
    }
}

impl<R: TableRow + 'static> Widget for Table<R> {
    fn space_demand(&self) -> (Demand, Demand) {
        let mut x_demands = vec![Demand::exact(0); R::num_columns()];
        let mut y_demand = Demand::zero();

        let mut row_iter = self.rows.iter().peekable();
        while let Some(row) = row_iter.next() {
            let mut row_max_y = Demand::exact(0);
            for (col_num, col) in R::columns().iter().enumerate() {
                let (x, y) = (col.access)(row).space_demand();
                x_demands[col_num].max_assign(x);
                row_max_y.max_assign(y)
            }
            y_demand += row_max_y;
            if row_iter.peek().is_some() {
                y_demand += Demand::exact(self.row_sep_style.height());
            }
        }

        //Account all separators between cols
        let x_demand = x_demands.iter().sum::<Demand>() + Demand::exact((x_demands.len() as u32 -1)*self.col_sep_style.width());
        (x_demand, y_demand)
    }
    fn draw(&mut self, window: Window) {
        let column_widths = self.layout_columns(&window);

        let mut window = window;
        let mut row_iter = self.rows.iter_mut().enumerate().peekable();
        while let Some((row_index, row)) = row_iter.next() {
            let height = row.height_demand().min;
            let (mut row_window, rest_window) = window.split_v(height);
            window = rest_window;

            if let (1, &SeparatingStyle::AlternatingStyle(modifier)) = (row_index%2, &self.row_sep_style) {
                row_window.modify_default_style(&modifier);
            }

            let mut iter = R::columns().iter().zip(column_widths.iter()).enumerate().peekable();
            while let Some((col_index, (col, &pos))) = iter.next() {
                let (mut cell_window, r) = row_window.split_h(pos);
                row_window = r;

                if let (1, &SeparatingStyle::AlternatingStyle(modifier)) = (col_index%2, &self.col_sep_style) {
                    cell_window.modify_default_style(&modifier);
                }

                if row_index as u32 == self.row_pos && col_index as u32 == self.col_pos {
                    cell_window.modify_default_style(&self.focused_style);
                }

                cell_window.clear(); // Fill background using new style
                (col.access_mut)(row).draw(cell_window);
                if let (Some(_), &SeparatingStyle::Draw(ref c)) = (iter.peek(), &self.col_sep_style) {
                    if row_window.get_width() > 0 {
                        let (mut sep_window, r) = row_window.split_h(c.width() as u32);
                        row_window = r;
                        sep_window.fill(c.clone());
                    }
                }
            }
            if let (Some(_), &SeparatingStyle::Draw(ref c)) = (row_iter.peek(), &self.row_sep_style) {
                let (mut sep_window, r) = window.split_v(1);
                window = r;
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
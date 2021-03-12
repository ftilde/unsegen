//! Higher level window manager like functionality using containers as the combination of widget and input concepts.
//!
//! Compose widgets into multi-widget applications using `Containers` and `ContainerManager` as the
//! analogon of a window manager.
//!
//! # Example:
//! ```no_run //tests do not provide a fully functional terminal
//! use unsegen::base::*;
//! use unsegen::container::*;
//! use unsegen::input::*;
//! use unsegen::widget::builtin::*;
//! use unsegen::widget::*;
//! use std::io::{stdin, stdout};
//!
//! struct Pager {
//!     buffer: LogViewer,
//! }
//!
//! impl Pager {
//!     fn new() -> Self {
//!         Pager {
//!             buffer: LogViewer::new(),
//!         }
//!     }
//! }
//!
//! impl Container<()> for Pager {
//!     fn input(&mut self, input: Input, _: &mut ()) -> Option<Input> {
//!         input
//!             .chain(
//!                 ScrollBehavior::new(&mut self.buffer)
//!                     .backwards_on(Key::Char('k'))
//!                     .forwards_on(Key::Char('j')),
//!             )
//!             .finish()
//!     }
//!     fn as_widget<'a>(&'a self) -> Box<dyn Widget + 'a> {
//!         Box::new(self.buffer.as_widget())
//!     }
//! }
//!
//! #[derive(Clone, PartialEq)]
//! enum Index {
//!     Left,
//!     Right,
//! }
//!
//! struct App {
//!     left: Pager,
//!     right: Pager,
//! }
//!
//! impl ContainerProvider for App {
//!     type Parameters = ();
//!     type Index = Index;
//!     fn get<'a, 'b: 'a>(&'b self, index: &'a Self::Index) -> &'b dyn Container<Self::Parameters> {
//!         match index {
//!             Index::Left => &self.left,
//!             Index::Right => &self.right,
//!         }
//!     }
//!     fn get_mut<'a, 'b: 'a>(
//!         &'b mut self,
//!         index: &'a Self::Index,
//!     ) -> &'b mut dyn Container<Self::Parameters> {
//!         match index {
//!             Index::Left => &mut self.left,
//!             Index::Right => &mut self.right,
//!         }
//!     }
//!     const DEFAULT_CONTAINER: Self::Index = Index::Left;
//! }
//!
//! fn main() {
//!     let stdout = stdout();
//!     let stdin = stdin();
//!     let stdin = stdin.lock();
//!
//!     let mut app = App {
//!         left: Pager::new(),
//!         right: Pager::new(),
//!     };
//!     let mut manager = ContainerManager::<App>::from_layout(Box::new(VSplit::new(vec![
//!         Box::new(Leaf::new(Index::Left)),
//!         Box::new(Leaf::new(Index::Right)),
//!     ])));
//!     let mut term = Terminal::new(stdout.lock()).unwrap();
//!
//!     for input in Input::read_all(stdin) {
//!         let input = input.unwrap();
//!         input
//!             .chain(manager.active_container_behavior(&mut app, &mut ()))
//!             .chain(
//!                 NavigateBehavior::new(&mut manager.navigatable(&mut app))
//!                     .left_on(Key::Char('h'))
//!                     .right_on(Key::Char('l')),
//!             );
//!         // Put more application logic here...
//!
//!         {
//!             let win = term.create_root_window();
//!             manager.draw(
//!                 win,
//!                 &mut app,
//!                 StyleModifier::new().fg_color(Color::Yellow),
//!                 RenderingHints::default(),
//!             )
//!         }
//!         term.present();
//!     }
//! }
//! ```
pub mod boxdrawing;

use self::boxdrawing::{LineCell, LineSegment, LineType};
use base::basic_types::*;
use base::{CursorTarget, StyleModifier, Window};
use input::{Behavior, Input, Navigatable, OperationResult};
use std::cell::Cell;
use std::cmp::{max, min};
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::convert::From;
use std::ops::Range;
use widget::layouts::layout_linearly;
use widget::{ColDemand, Demand2D, RenderingHints, RowDemand, Widget};

/// Extension to the widget trait to enable passing input to (active) widgets.
/// The parameter P can be used to manipulate global application state.
pub trait Container<P: ?Sized> {
    fn input(&mut self, input: Input, parameters: &mut P) -> Option<Input>;
    fn as_widget<'a>(&'a self) -> Box<dyn Widget + 'a>;
}

/// A ContainerProvider stores the individual components (`Container`s) of an application and
/// allows them to be retrieved based on an index.
///
/// Note that every possible value for `Self::Index` must correspond to a valid component. A good
/// choice for an Index is therefore an enum.
pub trait ContainerProvider {
    type Parameters;
    type Index: Clone + PartialEq;
    fn get<'a, 'b: 'a>(&'b self, index: &'a Self::Index) -> &'b dyn Container<Self::Parameters>;
    fn get_mut<'a, 'b: 'a>(
        &'b mut self,
        index: &'a Self::Index,
    ) -> &'b mut dyn Container<Self::Parameters>;
    const DEFAULT_CONTAINER: Self::Index;
}

/// A `Behavior` which can be used to pass input to the currently active container.
pub struct ActiveContainerBehavior<'a, 'b, 'c, 'd: 'a, C: ContainerProvider + 'a + 'b>
where
    C::Parameters: 'c,
{
    manager: &'a mut ContainerManager<'d, C>,
    provider: &'b mut C,
    parameters: &'c mut C::Parameters,
}

/// Pass input on to the currently active container.
impl<'a, 'b, 'c, 'd: 'a, C: ContainerProvider + 'a + 'b> Behavior
    for ActiveContainerBehavior<'a, 'b, 'c, 'd, C>
{
    fn input(self, i: Input) -> Option<Input> {
        i.chain(|i| {
            self.provider
                .get_mut(&self.manager.active)
                .input(i, self.parameters)
        })
        .finish()
    }
}

/// A simple rectangle with integer coordinates. Nothing to see here.
#[derive(Clone, Debug, PartialEq)]
pub struct Rectangle {
    pub x_range: Range<ColIndex>,
    pub y_range: Range<RowIndex>,
}

impl Rectangle {
    /// Calculate the total number of columns occupied by the rectangle.
    pub fn width(&self) -> Width {
        (self.x_range.end - self.x_range.start)
            .try_into_positive()
            .expect("range invariant")
    }
    /// Calculate the total number of rows occupied by the rectangle.
    pub fn height(&self) -> Height {
        (self.y_range.end - self.y_range.start)
            .try_into_positive()
            .expect("range invariant")
    }

    fn slice_range_x(&self, range: Range<ColIndex>) -> Rectangle {
        debug_assert!(
            self.x_range.start <= range.start && range.end <= self.x_range.end,
            "Invalid slice argument"
        );
        Rectangle {
            x_range: range,
            y_range: self.y_range.clone(),
        }
    }

    fn slice_range_y(&self, range: Range<RowIndex>) -> Rectangle {
        debug_assert!(
            self.y_range.start <= range.start && range.end <= self.y_range.end,
            "Invalid slice argument"
        );
        Rectangle {
            x_range: self.x_range.clone(),
            y_range: range,
        }
    }

    fn slice_line_x(&self, x: ColIndex) -> HorizontalLine {
        debug_assert!(
            self.x_range.start <= x && x <= self.x_range.end,
            "Invalid slice argument"
        );
        HorizontalLine {
            x: x,
            y_range: self.y_range.clone(),
        }
    }

    fn slice_line_y(&self, y: RowIndex) -> VerticalLine {
        debug_assert!(
            self.y_range.start <= y && y <= self.y_range.end,
            "Invalid slice argument"
        );
        VerticalLine {
            x_range: self.x_range.clone(),
            y: y,
        }
    }

    fn is_near_border(&self, x: ColIndex, y: RowIndex, dir: LineSegment) -> bool {
        let x_l = self.x_range.start - 1;
        let x_r = self.x_range.end;
        let y_l = self.y_range.start - 1;
        let y_r = self.y_range.end;

        let left = x_l == x;
        let right = x_r == x;
        let up = y_l == y;
        let down = y_r == y;
        if right && dir == LineSegment::Right
            || left && dir == LineSegment::Left
            || up && dir == LineSegment::Up
            || down && dir == LineSegment::Down
        {
            false
        } else {
            (right || left) && (y_l <= y && y <= y_r) || (up || down) && (x_l <= x && x <= x_r)
        }
    }
}

/// A single line occupying a number of cells in a row.
pub struct HorizontalLine {
    pub x: ColIndex,
    pub y_range: Range<RowIndex>,
}

/// A single line occupying a number of cells in a column.
pub struct VerticalLine {
    pub x_range: Range<ColIndex>,
    pub y: RowIndex,
}

/// An axis aligned line, either vertical or horizontal.
pub enum Line {
    Horizontal(HorizontalLine),
    Vertical(VerticalLine),
}

impl From<HorizontalLine> for Line {
    fn from(l: HorizontalLine) -> Self {
        Line::Horizontal(l)
    }
}

impl From<VerticalLine> for Line {
    fn from(l: VerticalLine) -> Self {
        Line::Vertical(l)
    }
}

/// A Layouter managing screen real estate for multiple containers
pub trait Layout<C: ContainerProvider> {
    /// Calculate the space demand required for all of the provided containers
    fn space_demand(&self, containers: &C) -> Demand2D;
    /// Specify how the provided containers should be layed out in the provided area, and how they
    /// should be separated by lines.
    ///
    /// Note that the implementor is strictly required to enforce that returned windows and lines
    /// DO NOT INTERSECT!
    fn layout(&self, available_area: Rectangle, containers: &C) -> LayoutOutput<C::Index>;
}

/// The result of a layouting operation for containers.
///
/// Required invariant: None of the windows or lines mutually intersect!
pub struct LayoutOutput<I: Clone> {
    /// A mapping from a container index to the screen area where the container will be drawn.
    pub windows: Vec<(I, Rectangle)>,
    /// A number of lines not directly associated with containers.
    ///
    /// (However, it is probably a good idea to use these to visually separate individual
    /// containers.)
    pub separators: Vec<Line>,
}

impl<I: Clone + PartialEq> LayoutOutput<I> {
    /// Create an empty `LayoutOutput`.
    fn new() -> Self {
        LayoutOutput {
            windows: Vec::new(),
            separators: Vec::new(),
        }
    }

    /// Add all windows and lines from the provided output to the current.
    fn add_child(&mut self, child: LayoutOutput<I>) {
        for (index, window) in child.windows {
            //self.windows.push((index, region.transform_to_outside_rectangle(window)));
            self.windows.push((index, window));
        }
        for separator in child.separators {
            //self.separators.push(region.transform_to_outside_line(separator));
            self.separators.push(separator);
        }
    }

    /// Retrieve the rectangle for the provided index
    fn get_rect_with_index(&self, index: I) -> Option<Rectangle> {
        self.windows
            .iter()
            .find(|&&(ref i, _)| *i == index)
            .map(|&(_, ref w)| w.clone())
    }
}

/// A `Leaf` in a `Layout`-tree.
///
/// It simply refers to a container by its index.
pub struct Leaf<C: ContainerProvider> {
    container_index: C::Index,
}

impl<C: ContainerProvider> Leaf<C> {
    /// Create the `Leaf` from a container index.
    pub fn new(index: C::Index) -> Self {
        Leaf {
            container_index: index,
        }
    }
}

impl<C: ContainerProvider> Layout<C> for Leaf<C> {
    fn space_demand(&self, containers: &C) -> Demand2D {
        containers
            .get(&self.container_index)
            .as_widget()
            .space_demand()
    }
    fn layout(&self, available_area: Rectangle, _: &C) -> LayoutOutput<C::Index> {
        let mut output = LayoutOutput::new();
        output
            .windows
            .push((self.container_index.clone(), available_area));
        output
    }
}

/// A `Layout` laying out all children horizontally, separated by vertical lines.
pub struct HSplit<'a, C: ContainerProvider> {
    elms: Vec<Box<dyn Layout<C> + 'a>>,
}

impl<'a, C: ContainerProvider> HSplit<'a, C> {
    /// Create a `HSplit` from its children.
    ///
    /// The order of children defines the drawing order from left to right.
    pub fn new(elms: Vec<Box<dyn Layout<C> + 'a>>) -> Self {
        HSplit { elms: elms }
    }
}

impl<'a, C: ContainerProvider> Layout<C> for HSplit<'a, C> {
    fn space_demand(&self, containers: &C) -> Demand2D {
        let mut total_x = ColDemand::exact(0);
        let mut total_y = RowDemand::exact(0);
        for e in self.elms.iter() {
            let demand2d = e.space_demand(containers);
            total_x = total_x + demand2d.width;
            total_y = total_y.max(demand2d.height);
        }
        total_x = total_x + ColDemand::exact(self.elms.len().checked_sub(1).unwrap_or(0));
        Demand2D {
            width: total_x,
            height: total_y,
        }
    }
    fn layout(&self, available_area: Rectangle, containers: &C) -> LayoutOutput<C::Index> {
        let separator_length = Width::new(1).unwrap();
        let horizontal_demands: Vec<ColDemand> = self
            .elms
            .iter()
            .map(|w| w.space_demand(containers).width)
            .collect();
        let assigned_spaces = layout_linearly(
            available_area.width(),
            separator_length,
            horizontal_demands.as_slice(),
        );
        let mut output = LayoutOutput::new();
        let mut p = available_area.x_range.start;
        for (elm, space) in self.elms.iter().zip(assigned_spaces.into_iter()) {
            let elm_rect = available_area.slice_range_x(p..(p + *space));
            output.add_child(elm.layout(elm_rect, containers));
            p += *space;

            if p < available_area.x_range.end {
                output
                    .separators
                    .push(available_area.slice_line_x(p).into());
                p += 1
            }
        }
        output
    }
}

/// A `Layout` laying out all children vertically, separated by Horizontal lines.
pub struct VSplit<'a, C: ContainerProvider> {
    elms: Vec<Box<dyn Layout<C> + 'a>>,
}

impl<'a, C: ContainerProvider> VSplit<'a, C> {
    /// Create a `VSplit` from its children.
    ///
    /// The order of children defines the drawing order from top to bottom.
    pub fn new(elms: Vec<Box<dyn Layout<C> + 'a>>) -> Self {
        VSplit { elms: elms }
    }
}

impl<'a, C: ContainerProvider> Layout<C> for VSplit<'a, C> {
    fn space_demand(&self, containers: &C) -> Demand2D {
        let mut total_x = ColDemand::exact(0);
        let mut total_y = RowDemand::exact(0);
        for e in self.elms.iter() {
            let demand2d = e.space_demand(containers);
            total_x = total_x.max(demand2d.width);
            total_y = total_y + demand2d.height;
        }
        total_y += RowDemand::exact(self.elms.len().checked_sub(1).unwrap_or(0));
        Demand2D {
            width: total_x,
            height: total_y,
        }
    }
    fn layout(&self, available_area: Rectangle, containers: &C) -> LayoutOutput<C::Index> {
        let separator_length = Height::new(1).unwrap();
        let vertical_demands: Vec<RowDemand> = self
            .elms
            .iter()
            .map(|w| w.space_demand(containers).height)
            .collect();
        let assigned_spaces = layout_linearly(
            available_area.height(),
            separator_length,
            vertical_demands.as_slice(),
        );
        let mut output = LayoutOutput::new();
        let mut p = available_area.y_range.start;
        for (elm, space) in self.elms.iter().zip(assigned_spaces.into_iter()) {
            let elm_rect = available_area.slice_range_y(p..(p + *space));
            output.add_child(elm.layout(elm_rect, containers));
            p += *space;

            if p < available_area.y_range.end {
                output
                    .separators
                    .push(available_area.slice_line_y(p).into());
                p += 1
            }
        }
        output
    }
}

/// A wrapper allowing for user defined modification of the currently active container using
/// `NavigateBehavior`.
pub struct NavigatableContainerManager<'a, 'b, 'd: 'a, C: ContainerProvider + 'a + 'b> {
    manager: &'a mut ContainerManager<'d, C>,
    provider: &'b mut C,
}

enum MovementDirection {
    Up,
    Down,
    Left,
    Right,
}

fn raw_range<T: AxisDimension>(range: &Range<AxisIndex<T>>) -> Range<i32> {
    range.start.raw_value()..range.end.raw_value()
}

impl<'a, 'b, 'd: 'a, C: ContainerProvider + 'a + 'b> NavigatableContainerManager<'a, 'b, 'd, C> {
    fn move_to(&mut self, direction: MovementDirection) -> OperationResult {
        let window_size = self.manager.last_window_size.get();
        let window_rect = Rectangle {
            x_range: 0.into()..window_size.0.from_origin(),
            y_range: 0.into()..window_size.1.from_origin(),
        };
        let layout_result = self.manager.layout.layout(window_rect, self.provider);
        let active_rect = layout_result
            .get_rect_with_index(self.manager.active.clone())
            .ok_or(())?;
        let best = layout_result
            .windows
            .iter()
            .filter_map(|&(ref candidate_index, ref candidate_rect)| {
                if *candidate_index == self.manager.active {
                    return None;
                }
                let (smaller_adjacent, greater_adjacent, active_range, candidate_range) =
                    match direction {
                        MovementDirection::Up => (
                            candidate_rect.y_range.end.raw_value(),
                            active_rect.y_range.start.raw_value(),
                            raw_range(&active_rect.x_range),
                            raw_range(&candidate_rect.x_range),
                        ),
                        MovementDirection::Down => (
                            active_rect.y_range.end.raw_value(),
                            candidate_rect.y_range.start.raw_value(),
                            raw_range(&active_rect.x_range),
                            raw_range(&candidate_rect.x_range),
                        ),
                        MovementDirection::Left => (
                            candidate_rect.x_range.end.raw_value(),
                            active_rect.x_range.start.raw_value(),
                            raw_range(&active_rect.y_range),
                            raw_range(&candidate_rect.y_range),
                        ),
                        MovementDirection::Right => (
                            active_rect.x_range.end.raw_value(),
                            candidate_rect.x_range.start.raw_value(),
                            raw_range(&active_rect.y_range),
                            raw_range(&candidate_rect.y_range),
                        ),
                    };
                if smaller_adjacent < greater_adjacent && greater_adjacent - smaller_adjacent == 1 {
                    // Rects are adjacent
                    let overlap = min(active_range.end, candidate_range.end)
                        .checked_sub(max(active_range.start, candidate_range.start))
                        .unwrap_or(0);
                    Some((overlap, candidate_index))
                } else {
                    None
                }
            })
            .max_by_key(|&(overlap, _)| overlap);

        if let Some((_, index)) = best {
            self.manager.active = index.clone();
            Ok(())
        } else {
            Err(())
        }
    }
}
impl<'a, 'b, 'd: 'a, C: ContainerProvider + 'a + 'b> Navigatable
    for NavigatableContainerManager<'a, 'b, 'd, C>
{
    fn move_up(&mut self) -> OperationResult {
        self.move_to(MovementDirection::Up)
    }
    fn move_down(&mut self) -> OperationResult {
        self.move_to(MovementDirection::Down)
    }
    fn move_left(&mut self) -> OperationResult {
        self.move_to(MovementDirection::Left)
    }
    fn move_right(&mut self) -> OperationResult {
        self.move_to(MovementDirection::Right)
    }
}

/// Something to draw lines on
struct LineCanvas {
    cells: BTreeMap<(ColIndex, RowIndex), LineCell>,
}

impl LineCanvas {
    fn new() -> Self {
        LineCanvas {
            cells: BTreeMap::new(),
        }
    }

    fn get_mut(&mut self, x: ColIndex, y: RowIndex) -> &mut LineCell {
        self.cells.entry((x, y)).or_insert(LineCell::empty())
    }

    fn into_iter(self) -> LineCanvasIter {
        LineCanvasIter {
            iter: self.cells.into_iter(),
        }
    }
}

struct LineCanvasIter {
    iter: btree_map::IntoIter<(ColIndex, RowIndex), LineCell>,
}

impl Iterator for LineCanvasIter {
    type Item = (ColIndex, RowIndex, LineCell);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|((x, y), c)| (x, y, c))
    }
}

/// Stores the layout of containers and manages and has a concept of an active container.
///
/// In some sense this is the analogon of a "window manager" for containers.
pub struct ContainerManager<'a, C: ContainerProvider> {
    layout: Box<dyn Layout<C> + 'a>,
    active: C::Index,
    last_window_size: Cell<(Width, Height)>,
}

impl<'a, C: ContainerProvider> ContainerManager<'a, C> {
    /// Create a `ContainerManager` from a given `Layout`. Initially, the default container is
    /// active.
    pub fn from_layout(layout_root: Box<dyn Layout<C> + 'a>) -> Self {
        ContainerManager {
            layout: layout_root,
            active: C::DEFAULT_CONTAINER.clone(),
            last_window_size: Cell::new((Width::new(100).unwrap(), Height::new(100).unwrap())),
        }
    }

    /// Draw all containers and separating lines onto the provided window.
    ///
    /// Use `border_style` to change how the lines will be drawn.
    ///
    /// `hints` will be passed on to containers, with the exception that only the currently active
    /// container can have an `active` hint.
    pub fn draw(
        &self,
        mut window: Window,
        provider: &mut C,
        border_style: StyleModifier,
        hints: RenderingHints,
    ) {
        self.last_window_size
            .set((window.get_width(), window.get_height()));

        let window_rect = Rectangle {
            x_range: 0.into()..window.get_width().from_origin(),
            y_range: 0.into()..window.get_height().from_origin(),
        };

        let layout_result = self.layout.layout(window_rect, provider);
        let active_rect = layout_result.get_rect_with_index(self.active.clone());

        for (index, rect) in layout_result.windows {
            let hints = if index == self.active {
                hints
            } else {
                hints.active(false)
            };

            provider
                .get_mut(&index)
                .as_widget()
                .draw(window.create_subwindow(rect.x_range, rect.y_range), hints);
        }

        let get_line_type = |x, y, s| {
            if let &Some(ref active_rect) = &active_rect {
                if active_rect.is_near_border(x, y, s) {
                    LineType::Thick
                } else {
                    LineType::Thin
                }
            } else {
                LineType::Thin
            }
        };

        let mut line_canvas = LineCanvas::new();
        for line in layout_result.separators {
            match line {
                Line::Horizontal(HorizontalLine { x, y_range }) => {
                    line_canvas.get_mut(x, y_range.start - 1).set(
                        LineSegment::Down,
                        get_line_type(x, y_range.start - 1, LineSegment::Down),
                    );
                    for y in IndexRange(y_range.start..y_range.end) {
                        line_canvas
                            .get_mut(x, y)
                            .set(LineSegment::Up, get_line_type(x, y, LineSegment::Up))
                            .set(LineSegment::Down, get_line_type(x, y, LineSegment::Down));
                    }
                    line_canvas.get_mut(x, y_range.end).set(
                        LineSegment::Up,
                        get_line_type(x, y_range.end, LineSegment::Up),
                    );
                }
                Line::Vertical(VerticalLine { x_range, y }) => {
                    line_canvas.get_mut(x_range.start - 1, y).set(
                        LineSegment::Right,
                        get_line_type(x_range.start - 1, y, LineSegment::Right),
                    );
                    for x in IndexRange(x_range.start..x_range.end) {
                        line_canvas
                            .get_mut(x, y)
                            .set(LineSegment::Right, get_line_type(x, y, LineSegment::Right))
                            .set(LineSegment::Left, get_line_type(x, y, LineSegment::Left));
                    }
                    line_canvas.get_mut(x_range.end, y).set(
                        LineSegment::Left,
                        get_line_type(x_range.end, y, LineSegment::Left),
                    );
                }
            }
        }

        for (x, y, cell) in line_canvas.into_iter() {
            if let Some(styled_cluster) = window.get_cell_mut(x, y) {
                styled_cluster.grapheme_cluster = cell.to_grapheme_cluster();
                border_style.modify(&mut styled_cluster.style);
            }
        }
    }

    /// Allow the active container to be changed using a `NavigateBehavior`.
    pub fn navigatable<'b, 'c>(
        &'b mut self,
        provider: &'c mut C,
    ) -> NavigatableContainerManager<'b, 'c, 'a, C> {
        NavigatableContainerManager::<C> {
            manager: self,
            provider: provider,
        }
    }

    /// Behavior that passes all input to the currently active container.
    pub fn active_container_behavior<'b, 'c, 'd>(
        &'b mut self,
        provider: &'c mut C,
        parameters: &'d mut C::Parameters,
    ) -> ActiveContainerBehavior<'b, 'c, 'd, 'a, C> {
        ActiveContainerBehavior {
            manager: self,
            provider: provider,
            parameters: parameters,
        }
    }

    /// Get the index of the currently active container.
    pub fn active(&self) -> C::Index {
        self.active.clone()
    }

    /// Set the currently active container using its Index.
    pub fn set_active(&mut self, i: C::Index) {
        self.active = i;
    }
}

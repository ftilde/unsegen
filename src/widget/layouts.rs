//! Basic linear layouting for `Widget`s.
use super::{ColDemand, Demand, Demand2D, RenderingHints, RowDemand, Widget};
use base::basic_types::*;
use base::{GraphemeCluster, StyleModifier, Window};
use std::cmp::Ord;
use std::fmt::Debug;

/// Compute assigned lengths for the given demands in one dimension of size `available_space`.
///
/// Between each length, a gap of `separator_width` will be assumed.
///
/// This (somewhat ad-hoc) algorithm tries to satisfy these requirements in the following order:
///
/// 1. Each demand should be treated equally.
/// 2. Every demands minimum should be honored.
/// 3. Each demand should be treated equally, but the assigned length shall not exceed the maximum.
/// 4. All space will be distributed.
pub fn layout_linearly<T: AxisDimension + Ord + Debug + Clone>(
    available_space: PositiveAxisDiff<T>,
    separator_width: PositiveAxisDiff<T>,
    demands: &[Demand<T>],
    weights: &[f64],
) -> Box<[PositiveAxisDiff<T>]> {
    //eprintln!("av {}, sep {}, dem, {:?}", available_space, separator_width, demands);
    assert_eq!(demands.len(), weights.len());

    let mut assigned_spaces = vec![0.0; demands.len()].into_boxed_slice();

    struct DemandF {
        min: f64,
        max: f64,
    }

    let demands = demands
        .iter()
        .map(|d| DemandF {
            min: d.min.raw_value() as f64,
            max: d.max.unwrap_or(available_space).raw_value() as f64,
        })
        .collect::<Vec<_>>();

    // Reserve space for separators
    let diff = available_space - separator_width * demands.len().saturating_sub(1);
    if diff < 0 {
        return vec![PositiveAxisDiff::new(0).unwrap(); demands.len()].into_boxed_slice();
    }

    let total = diff.try_into_positive().unwrap().raw_value() as f64;

    // Try to fullfil all min demands fairly according to weight
    {
        let mut total_unfinished = total;
        let mut unfulfilled_min = (0..demands.len()).into_iter().collect::<Vec<usize>>();

        while !unfulfilled_min.is_empty() {
            let weight_sum: f64 = unfulfilled_min.iter().map(|i| weights[*i]).sum();

            let mut still_unfullfilled = Vec::<usize>::new();
            let to_distribute = total_unfinished;
            for i in &unfulfilled_min {
                let i = *i;

                let demand = &demands[i];
                let weight = weights[i];
                let assigned_space = &mut assigned_spaces[i];

                let budget_coeff: f64 = if weight_sum > 0.0 {
                    weight / weight_sum
                } else {
                    1.0
                };
                let budget = to_distribute * budget_coeff;

                let max = demand.min;
                let space = max.min(budget);
                *assigned_space = space;

                if *assigned_space < max {
                    still_unfullfilled.push(i);
                } else {
                    total_unfinished -= *assigned_space;
                }
            }
            if still_unfullfilled.len() == unfulfilled_min.len() {
                break;
            }
            unfulfilled_min = still_unfullfilled;
        }
    }

    // Try to fullfil max demands, if not in conflict with min demands
    {
        // Collect all widgets that have at least the min demand met so far.
        let mut total_unfinished = total;
        let mut unfinished = Vec::new();
        for i in 0..demands.len() {
            let demand = &demands[i];
            let assigned = assigned_spaces[i];
            if demand.min <= assigned && assigned < demand.max {
                unfinished.push(i);
            } else {
                total_unfinished -= assigned;
            }
        }

        // Then remove all that would get less than min demand in weighted distribution
        {
            let weight_sum: f64 = unfinished.iter().map(|i| weights[*i]).sum();
            let mut still_unfinished = Vec::<usize>::new();

            let to_distribute = total_unfinished;
            for i in &unfinished {
                let i = *i;

                let demand = &demands[i];
                let weight = weights[i];

                let budget_coeff: f64 = if weight_sum > 0.0 {
                    weight / weight_sum
                } else {
                    1.0
                };
                let budget = to_distribute * budget_coeff;

                if budget > demand.min {
                    still_unfinished.push(i);
                } else {
                    total_unfinished -= demand.min;
                }
            }
            unfinished = still_unfinished;
        }

        // Distribute the remaining space according to weights
        while !unfinished.is_empty() {
            let weight_sum: f64 = unfinished.iter().map(|i| weights[*i]).sum();

            let mut still_unfinished = Vec::<usize>::new();
            let to_distribute = total_unfinished;
            for i in &unfinished {
                let i = *i;

                let demand = &demands[i];
                let weight = weights[i];
                let assigned_space = &mut assigned_spaces[i];

                let budget_coeff: f64 = if weight_sum > 0.0 {
                    weight / weight_sum
                } else {
                    1.0
                };
                let budget = to_distribute * budget_coeff;

                let max = demand.max;
                let space = max.min(budget);
                *assigned_space = space;

                if *assigned_space < max {
                    still_unfinished.push(i);
                } else {
                    total_unfinished -= *assigned_space;
                }
            }
            if still_unfinished.len() == unfinished.len() {
                break;
            }
            unfinished = still_unfinished;
        }
    }

    let mut assigned_int = assigned_spaces
        .into_iter()
        .map(|f| PositiveAxisDiff::<T>::new_unchecked(*f as i32))
        .collect::<Vec<_>>();

    let total_assigned: PositiveAxisDiff<T> = assigned_int.iter().sum();
    let total_demand: AxisDiff<T> = demands.iter().map(|d| AxisDiff::new(d.max as i32)).sum();
    let mut still_to_assign = (diff - total_assigned).min(total_demand);

    // Distribute spaces accumulated through rounding errors
    {
        // Collect not completely fulfilled rewards
        let mut unfinished = (0..demands.len())
            .into_iter()
            .filter(|i| {
                let s = &assigned_int[*i];
                let demand = &demands[*i];
                s.raw_value() < demand.max as i32
            })
            .collect::<Vec<usize>>();

        while !unfinished.is_empty() {
            let mut still_unfinished = Vec::<usize>::new();
            for i in unfinished {
                if still_to_assign == 0 {
                    break;
                }

                let demand = &demands[i];
                let s = &mut assigned_int[i];

                *s += 1;
                still_to_assign -= 1;

                if s.raw_value() < demand.max as i32 {
                    still_unfinished.push(i);
                }
            }
            unfinished = still_unfinished;
        }
    }

    assigned_int.into_boxed_slice()
}

/// Draw the widgets in the given window in a linear layout.
fn draw_linearly<'a, T: AxisDimension + Ord + Debug + Copy, S, L, M, D>(
    window: Window,
    widgets: &[Box<dyn Widget + 'a>],
    weights: &[f64],
    rendering_hints: &[RenderingHints],
    separating_style: &SeparatingStyle,
    split: S,
    window_length: L,
    separator_length: M,
    demand_dimension: D,
) where
    S: Fn(Window, AxisIndex<T>) -> (Window, Window),
    L: Fn(&Window) -> PositiveAxisDiff<T>,
    M: Fn(&SeparatingStyle) -> PositiveAxisDiff<T>,
    D: Fn(Demand2D) -> Demand<T>,
{
    assert_eq!(widgets.len(), weights.len());
    assert_eq!(widgets.len(), rendering_hints.len());
    let separator_length = separator_length(separating_style);
    let demands: Vec<Demand<T>> = widgets
        .iter()
        .map(|w| demand_dimension(w.space_demand()))
        .collect();
    let assigned_spaces = layout_linearly(
        window_length(&window),
        separator_length,
        demands.as_slice(),
        weights,
    );

    debug_assert!(
        widgets.len() == assigned_spaces.len(),
        "widgets and spaces len mismatch"
    );

    let mut rest_window = window;
    let mut iter = widgets
        .iter()
        .zip(rendering_hints.iter())
        .zip(assigned_spaces.iter())
        .enumerate()
        .peekable();
    while let Some((i, ((w, hint), &pos))) = iter.next() {
        let (mut window, r) = split(rest_window, pos.from_origin());
        rest_window = r;
        if let (1, &SeparatingStyle::AlternatingStyle(modifier)) = (i % 2, separating_style) {
            window.modify_default_style(modifier);
        }
        window.clear(); // Fill background using new style
        w.draw(window, *hint);
        if let (Some(_), &SeparatingStyle::Draw(ref c)) = (iter.peek(), separating_style) {
            if window_length(&rest_window) > 0 {
                let (mut window, r) = split(rest_window, separator_length.from_origin());
                rest_window = r;
                window.fill(c.clone());
            }
        }
    }
}

pub struct HLayout<'a> {
    separating_style: SeparatingStyle,
    widgets: Vec<Box<dyn Widget + 'a>>,
    weights: Vec<f64>,
}

impl<'a> HLayout<'a> {
    pub fn new() -> Self {
        HLayout {
            separating_style: SeparatingStyle::None,
            widgets: Vec::new(),
            weights: Vec::new(),
        }
    }

    pub fn separator(self, separator: GraphemeCluster) -> Self {
        self.separating_style(SeparatingStyle::Draw(separator))
    }

    pub fn alterating(self, style_modifier: StyleModifier) -> Self {
        self.separating_style(SeparatingStyle::AlternatingStyle(style_modifier))
    }

    pub fn separating_style(mut self, style: SeparatingStyle) -> Self {
        self.separating_style = style;
        self
    }

    pub fn widget<W: Widget + 'a>(self, t: W) -> Self {
        self.widget_weighted(t, 1.0)
    }

    pub fn widget_weighted<W: Widget + 'a>(mut self, t: W, weight: f64) -> Self {
        self.widgets.push(Box::new(t));
        self.weights.push(weight);
        self
    }
}

impl<'a> Widget for HLayout<'a> {
    fn space_demand(&self) -> Demand2D {
        let mut total_x = ColDemand::exact(0);
        let mut total_y = RowDemand::exact(0);
        let mut n_elements = 0;
        for w in self.widgets.iter() {
            let demand2d = w.space_demand();
            total_x = total_x + demand2d.width;
            total_y = total_y.max(demand2d.height);
            n_elements += 1;
        }
        if let SeparatingStyle::Draw(_) = self.separating_style {
            total_x += Demand::exact(n_elements);
        }
        Demand2D {
            width: total_x,
            height: total_y,
        }
    }
    fn draw(&self, window: Window, hints: RenderingHints) {
        let hints = std::iter::repeat(hints)
            .take(self.widgets.len())
            .collect::<Vec<_>>();
        draw_linearly(
            window,
            &self.widgets,
            &self.weights,
            &hints,
            &self.separating_style,
            |w, p| w.split(p).expect("valid split pos"),
            |w| w.get_width(),
            SeparatingStyle::width,
            |d| d.width,
        );
    }
}

pub struct VLayout<'a> {
    separating_style: SeparatingStyle,
    widgets: Vec<Box<dyn Widget + 'a>>,
    weights: Vec<f64>,
}

impl<'a> VLayout<'a> {
    pub fn new() -> Self {
        VLayout {
            separating_style: SeparatingStyle::None,
            widgets: Vec::new(),
            weights: Vec::new(),
        }
    }

    pub fn separator(self, separator: GraphemeCluster) -> Self {
        self.separating_style(SeparatingStyle::Draw(separator))
    }

    pub fn alterating(self, style_modifier: StyleModifier) -> Self {
        self.separating_style(SeparatingStyle::AlternatingStyle(style_modifier))
    }

    pub fn separating_style(mut self, style: SeparatingStyle) -> Self {
        self.separating_style = style;
        self
    }

    pub fn widget<W: Widget + 'a>(self, t: W) -> Self {
        self.widget_weighted(t, 1.0)
    }

    pub fn widget_weighted<W: Widget + 'a>(mut self, t: W, weight: f64) -> Self {
        self.widgets.push(Box::new(t));
        self.weights.push(weight);
        self
    }
}

impl<'a> Widget for VLayout<'a> {
    fn space_demand(&self) -> Demand2D {
        let mut total_x = Demand::exact(0);
        let mut total_y = Demand::exact(0);
        let mut n_elements = 0;
        for w in self.widgets.iter() {
            let demand2d = w.space_demand();
            total_x = total_x.max(demand2d.width);
            total_y = total_y + demand2d.height;
            n_elements += 1;
        }
        if let SeparatingStyle::Draw(_) = self.separating_style {
            total_y = total_y + Demand::exact(n_elements);
        }
        Demand2D {
            width: total_x,
            height: total_y,
        }
    }

    /// Draw the given widgets to the window, from top to bottom.
    fn draw(&self, window: Window, hints: RenderingHints) {
        let hints = std::iter::repeat(hints)
            .take(self.widgets.len())
            .collect::<Vec<_>>();
        draw_linearly(
            window,
            &self.widgets,
            &self.weights,
            &hints,
            &self.separating_style,
            |w, p| w.split(p).expect("valid split pos"),
            |w| w.get_height(),
            SeparatingStyle::height,
            |d| d.height,
        );
    }
}

/// Variants on how to distinguish two neighboring widgets when drawing them to a window.
#[derive(Clone)]
pub enum SeparatingStyle {
    /// Do nothing to distinguish them
    None,
    /// Modify the style of every second widget
    AlternatingStyle(StyleModifier),
    /// Draw a line using the specified GraphemeCluster
    Draw(GraphemeCluster),
}
impl SeparatingStyle {
    /// The required additional width when using this style to separate widgets in a horizontal
    /// layout.
    pub fn width(&self) -> Width {
        match self {
            &SeparatingStyle::None => Width::new(0).unwrap(),
            &SeparatingStyle::AlternatingStyle(_) => Width::new(0).unwrap(),
            &SeparatingStyle::Draw(ref cluster) => cluster.width().into(),
        }
    }
    /// The required additional height when using this style to separate widgets in a vertical
    /// layout.
    pub fn height(&self) -> Height {
        match self {
            &SeparatingStyle::None => Height::new(0).unwrap(),
            &SeparatingStyle::AlternatingStyle(_) => Height::new(0).unwrap(),
            &SeparatingStyle::Draw(_) => Height::new(1).unwrap(),
        }
    }
}

#[cfg(test)]
mod test {
    // for fuzzing tests
    extern crate rand;
    use self::rand::Rng;

    use super::*;
    use base::test::FakeTerminal;

    struct FakeWidget {
        space_demand: Demand2D,
        fill_char: char,
    }
    impl FakeWidget {
        fn new(space_demand: (ColDemand, RowDemand)) -> Self {
            Self::with_fill_char(space_demand, '_')
        }
        fn with_fill_char(space_demand: (ColDemand, RowDemand), fill_char: char) -> Self {
            FakeWidget {
                space_demand: Demand2D {
                    width: space_demand.0,
                    height: space_demand.1,
                },
                fill_char: fill_char,
            }
        }
    }
    impl Widget for FakeWidget {
        fn space_demand(&self) -> Demand2D {
            self.space_demand
        }
        fn draw(&self, mut window: Window, _: RenderingHints) {
            window.fill(GraphemeCluster::try_from(self.fill_char).unwrap());
        }
    }

    #[track_caller]
    fn assert_eq_boxed_slices(b1: Box<[Width]>, b2: Box<[i32]>, description: &str) {
        let b2 = b2
            .iter()
            .map(|&i| Width::new(i).unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert_eq!(b1, b2, "{}", description);
    }

    fn w(i: i32) -> Width {
        Width::new(i).unwrap()
    }

    fn ll_unweighted<T: AxisDimension + Ord + Debug + Clone>(
        available_space: PositiveAxisDiff<T>,
        separator_width: PositiveAxisDiff<T>,
        demands: &[Demand<T>],
    ) -> Box<[PositiveAxisDiff<T>]> {
        let weights = std::iter::repeat(1.0)
            .take(demands.len())
            .collect::<Vec<_>>();
        layout_linearly(available_space, separator_width, demands, &weights)
    }

    #[test]
    fn test_layout_linearly_exact() {
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::exact(1), Demand::exact(2)]),
            Box::new([1, 2]),
            "some left",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::exact(1), Demand::exact(3)]),
            Box::new([1, 3]),
            "exact",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::exact(2), Demand::exact(3)]),
            Box::new([2, 2]),
            "less for 2nd",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::exact(5), Demand::exact(3)]),
            Box::new([2, 2]),
            "not enough for min",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(5), w(0), &[Demand::exact(5), Demand::exact(3)]),
            Box::new([3, 2]),
            "not enough for min unequal",
        );
    }

    #[test]
    fn test_layout_linearly_weighted_less_than_min() {
        assert_eq_boxed_slices(
            layout_linearly(
                w(4),
                w(0),
                &[Demand::at_least(3), Demand::at_least(5)],
                &[1.0, 1.0],
            ),
            Box::new([2, 2]),
            "equal",
        );
        assert_eq_boxed_slices(
            layout_linearly(
                w(10),
                w(0),
                &[Demand::at_least(3), Demand::at_least(5)],
                &[2.0, 3.0],
            ),
            Box::new([4, 6]),
            "uneven",
        );
        assert_eq_boxed_slices(
            layout_linearly(
                w(4),
                w(0),
                &[Demand::at_least(3), Demand::at_least(5)],
                &[0.0, 1.0],
            ),
            Box::new([0, 4]),
            "one zero",
        );
        assert_eq_boxed_slices(
            layout_linearly(
                w(6),
                w(0),
                &[Demand::at_least(3), Demand::at_least(5)],
                &[0.0, 1.0],
            ),
            Box::new([1, 5]),
            "one zero, partially fulfilled",
        );
    }

    #[test]
    fn test_layout_linearly_weighted_between_min_max() {
        assert_eq_boxed_slices(
            layout_linearly(
                w(4),
                w(0),
                &[Demand::from_to(1, 5), Demand::from_to(1, 4)],
                &[1.0, 1.0],
            ),
            Box::new([2, 2]),
            "equal",
        );
        assert_eq_boxed_slices(
            layout_linearly(
                w(10),
                w(0),
                &[Demand::from_to(1, 10), Demand::from_to(1, 10)],
                &[3.0, 2.0],
            ),
            Box::new([6, 4]),
            "uneven",
        );
        assert_eq_boxed_slices(
            layout_linearly(
                w(5),
                w(0),
                &[Demand::from_to(1, 10), Demand::from_to(1, 10)],
                &[0.0, 1.0],
            ),
            Box::new([1, 4]),
            "one zero",
        );
    }

    #[test]
    fn test_layout_linearly_from_to() {
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::from_to(1, 2), Demand::from_to(1, 2)]),
            Box::new([2, 2]),
            "both hit max",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::from_to(1, 2), Demand::from_to(1, 3)]),
            Box::new([2, 2]),
            "less for 2nd",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::from_to(5, 6), Demand::from_to(1, 4)]),
            Box::new([3, 1]),
            "not enough for min of first",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::from_to(1, 5), Demand::from_to(1, 4)]),
            Box::new([2, 2]),
            "both not full",
        );
    }

    #[test]
    fn test_layout_linearly_from_at_least() {
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::at_least(1), Demand::at_least(1)]),
            Box::new([2, 2]),
            "more for both",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::at_least(1), Demand::at_least(2)]),
            Box::new([2, 2]),
            "more for 1st, exact for 2nd",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::at_least(2), Demand::at_least(2)]),
            Box::new([2, 2]),
            "exact for both",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(4), w(0), &[Demand::at_least(5), Demand::at_least(2)]),
            Box::new([2, 2]),
            "not enough for min",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(5), w(0), &[Demand::at_least(5), Demand::at_least(2)]),
            Box::new([3, 2]),
            "not enough for min unequal",
        );
    }

    #[test]
    fn test_layout_linearly_mixed() {
        assert_eq_boxed_slices(
            ll_unweighted(w(10), w(0), &[Demand::exact(3), Demand::at_least(1)]),
            Box::new([3, 7]),
            "exact, 2nd takes rest, no separator",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(10), w(1), &[Demand::exact(3), Demand::at_least(1)]),
            Box::new([3, 6]),
            "exact, 2nd takes rest, separator",
        );
        assert_eq_boxed_slices(
            ll_unweighted(w(10), w(0), &[Demand::from_to(1, 2), Demand::at_least(1)]),
            Box::new([2, 8]),
            "from_to, 2nd takes rest",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(1, 2), Demand::exact(3), Demand::at_least(1)],
            ),
            Box::new([2, 3, 5]),
            "misc 1",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(5, 6), Demand::exact(5), Demand::at_least(5)],
            ),
            Box::new([4, 3, 3]),
            "misc 2",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(4, 6), Demand::exact(4), Demand::at_least(3)],
            ),
            Box::new([4, 3, 3]),
            "misc 3",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(3, 6), Demand::exact(4), Demand::at_least(3)],
            ),
            Box::new([3, 4, 3]),
            "misc 4",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(3, 6), Demand::exact(3), Demand::at_least(3)],
            ),
            Box::new([4, 3, 3]),
            "misc 5",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(2, 4), Demand::exact(2), Demand::at_least(3)],
            ),
            Box::new([4, 2, 4]),
            "misc 6",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(2, 4), Demand::exact(2), Demand::exact(3)],
            ),
            Box::new([4, 2, 3]),
            "misc 7",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(2, 4), Demand::exact(2), Demand::at_least(4)],
            ),
            Box::new([4, 2, 4]),
            "misc 8",
        );
        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[
                    Demand::from_to(2, 3),
                    Demand::at_least(2),
                    Demand::at_least(2),
                ],
            ),
            Box::new([3, 4, 3]),
            "misc 9",
        );

        assert_eq_boxed_slices(
            ll_unweighted(w(82), w(1), &[Demand::at_least(4), Demand::at_least(51)]),
            Box::new([30, 51]),
            "misc 10",
        );

        assert_eq_boxed_slices(
            ll_unweighted(
                w(10),
                w(0),
                &[Demand::from_to(6, 6), Demand::exact(4), Demand::at_least(2)],
            ),
            Box::new([4, 4, 2]),
            "misc 11",
        );
    }

    #[track_caller]
    fn aeq_horizontal_layout_space_demand(
        widgets: Vec<FakeWidget>,
        solution: (ColDemand, RowDemand),
    ) {
        let demand2d = Demand2D {
            width: solution.0,
            height: solution.1,
        };
        let mut layout = HLayout::new();
        for widget in widgets {
            layout = layout.widget(widget);
        }
        assert_eq!(layout.space_demand(), demand2d);
    }
    #[test]
    fn test_horizontal_layout_space_demand() {
        aeq_horizontal_layout_space_demand(
            vec![
                FakeWidget::new((Demand::exact(1), Demand::exact(2))),
                FakeWidget::new((Demand::exact(1), Demand::exact(2))),
            ],
            (Demand::exact(2), Demand::exact(2)),
        );
        aeq_horizontal_layout_space_demand(
            vec![
                FakeWidget::new((Demand::from_to(1, 2), Demand::from_to(1, 3))),
                FakeWidget::new((Demand::exact(1), Demand::exact(2))),
            ],
            (Demand::from_to(2, 3), Demand::from_to(2, 3)),
        );
        aeq_horizontal_layout_space_demand(
            vec![
                FakeWidget::new((Demand::at_least(3), Demand::at_least(3))),
                FakeWidget::new((Demand::exact(1), Demand::exact(5))),
            ],
            (Demand::at_least(4), Demand::at_least(5)),
        );
    }
    #[track_caller]
    fn aeq_horizontal_layout_draw(
        terminal_size: (u32, u32),
        widgets: Vec<FakeWidget>,
        solution: &str,
    ) {
        let mut term = FakeTerminal::with_size(terminal_size);
        let mut layout = HLayout::new();
        for widget in widgets {
            layout = layout.widget(widget);
        }
        layout.draw(term.create_root_window(), RenderingHints::default());
        assert_eq!(
            term,
            FakeTerminal::from_str(terminal_size, solution).expect("term from str"),
            "got <=> expected"
        );
    }
    #[test]
    fn test_horizontal_layout_draw() {
        aeq_horizontal_layout_draw(
            (4, 1),
            vec![
                FakeWidget::with_fill_char((Demand::exact(2), Demand::exact(1)), '1'),
                FakeWidget::with_fill_char((Demand::exact(2), Demand::exact(1)), '2'),
            ],
            "1122",
        );
        aeq_horizontal_layout_draw(
            (4, 1),
            vec![
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(1)), '1'),
                FakeWidget::with_fill_char((Demand::at_least(2), Demand::exact(1)), '2'),
            ],
            "1222",
        );
        aeq_horizontal_layout_draw(
            (4, 2),
            vec![
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(1)), '1'),
                FakeWidget::with_fill_char((Demand::at_least(2), Demand::exact(2)), '2'),
            ],
            "1222 1222",
        );
        aeq_horizontal_layout_draw(
            (8, 1),
            vec![
                FakeWidget::with_fill_char((Demand::at_least(1), Demand::at_least(1)), '1'),
                FakeWidget::with_fill_char((Demand::at_least(3), Demand::exact(3)), '2'),
            ],
            "11112222",
        );
    }

    #[track_caller]
    fn aeq_vertical_layout_space_demand(
        widgets: Vec<FakeWidget>,
        solution: (ColDemand, RowDemand),
    ) {
        let demand2d = Demand2D {
            width: solution.0,
            height: solution.1,
        };
        let mut layout = VLayout::new();
        for widget in widgets {
            layout = layout.widget(widget);
        }
        assert_eq!(layout.space_demand(), demand2d);
    }
    #[test]
    fn test_vertical_layout_space_demand() {
        aeq_vertical_layout_space_demand(
            vec![
                FakeWidget::new((Demand::exact(2), Demand::exact(1))),
                FakeWidget::new((Demand::exact(2), Demand::exact(1))),
            ],
            (Demand::exact(2), Demand::exact(2)),
        );
        aeq_vertical_layout_space_demand(
            vec![
                FakeWidget::new((Demand::from_to(1, 3), Demand::from_to(1, 2))),
                FakeWidget::new((Demand::exact(2), Demand::exact(1))),
            ],
            (Demand::from_to(2, 3), Demand::from_to(2, 3)),
        );
        aeq_vertical_layout_space_demand(
            vec![
                FakeWidget::new((Demand::at_least(3), Demand::at_least(3))),
                FakeWidget::new((Demand::exact(5), Demand::exact(1))),
            ],
            (Demand::at_least(5), Demand::at_least(4)),
        );
    }
    #[track_caller]
    fn aeq_vertical_layout_draw(
        terminal_size: (u32, u32),
        widgets: Vec<FakeWidget>,
        solution: &str,
    ) {
        let mut term = FakeTerminal::with_size(terminal_size);
        let mut layout = VLayout::new();
        for widget in widgets {
            layout = layout.widget(widget);
        }
        layout.draw(term.create_root_window(), RenderingHints::default());
        assert_eq!(
            term,
            FakeTerminal::from_str(terminal_size, solution).expect("term from str")
        );
    }
    #[test]
    fn test_vertical_layout_draw() {
        aeq_vertical_layout_draw(
            (1, 4),
            vec![
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(2)), '1'),
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(2)), '2'),
            ],
            "1 1 2 2",
        );
        aeq_vertical_layout_draw(
            (1, 4),
            vec![
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(1)), '1'),
                FakeWidget::with_fill_char((Demand::exact(1), Demand::at_least(2)), '2'),
            ],
            "1 2 2 2",
        );
        aeq_vertical_layout_draw(
            (2, 4),
            vec![
                FakeWidget::with_fill_char((Demand::exact(1), Demand::exact(1)), '1'),
                FakeWidget::with_fill_char((Demand::exact(2), Demand::at_least(2)), '2'),
            ],
            "11 22 22 22",
        );
        aeq_vertical_layout_draw(
            (1, 8),
            vec![
                FakeWidget::with_fill_char((Demand::at_least(2), Demand::at_least(2)), '1'),
                FakeWidget::with_fill_char((Demand::at_least(1), Demand::at_least(1)), '2'),
            ],
            "1 1 1 1 2 2 2 2",
        );
    }

    #[test]
    fn fuzz_layout_linearly() {
        let fuzz_iterations = 10000;
        let max_widgets = 10;
        let max_space = 1000;
        let max_separator_size = 5;

        let mut rng = rand::thread_rng();
        for _ in 0..fuzz_iterations {
            let mut demands = Vec::new();
            let mut weights = Vec::new();
            for _ in 0..max_widgets {
                let min = w(rng.gen_range(0, max_space));
                let demand = if rng.gen() {
                    Demand::from_to(min, w(rng.gen_range(min.raw_value(), max_space)))
                } else {
                    Demand::at_least(min)
                };
                demands.push(demand);
                weights.push(rng.gen_range(0.0, 1.0));
            }

            let space = rng.gen_range(0, max_space);
            let separator_size = rng.gen_range(0, max_separator_size);
            let layout = layout_linearly(w(space), w(separator_size), demands.as_slice(), &weights);

            let separator_space = (demands.len() as i32 - 1) * separator_size;

            let assigned: i32 = layout.iter().map(|l| l.raw_value()).sum();

            if assigned > 0 {
                assert!(space >= assigned + separator_space);
            }
        }
    }
}

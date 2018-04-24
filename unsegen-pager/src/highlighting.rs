use unsegen::base::{Color, ModifyMode, StyleModifier, TextFormatModifier};
use unsegen::widget::LineIndex;

use syntect::parsing::{ParseState, ScopeStack, SyntaxDefinition};
use syntect::highlighting;
use super::PagerLine;

use syntect::highlighting::Theme;

pub struct HighlightInfo {
    style_changes: Vec<Vec<(usize, StyleModifier)>>,
    default_style: StyleModifier,
    no_change: Vec<(usize, StyleModifier)>,
}

impl HighlightInfo {
    pub fn none() -> Self {
        HighlightInfo {
            style_changes: Vec::new(),
            default_style: StyleModifier::none(),
            no_change: Vec::new(),
        }
    }

    pub fn get_info_for_line<L: Into<LineIndex>>(&self, l: L) -> &Vec<(usize, StyleModifier)> {
        self.style_changes
            .get(l.into().0)
            .unwrap_or(&self.no_change)
    }

    pub fn default_style(&self) -> StyleModifier {
        self.default_style
    }
}

pub trait Highlighter {
    fn highlight<'a, L: Iterator<Item = &'a PagerLine>>(&self, lines: L) -> HighlightInfo;
}

pub struct SyntectHighlighter<'a> {
    base_state: ParseState,
    theme: &'a Theme,
}

impl<'a> SyntectHighlighter<'a> {
    pub fn new(syntax: &SyntaxDefinition, theme: &'a highlighting::Theme) -> Self {
        SyntectHighlighter {
            base_state: ParseState::new(syntax),
            theme: theme,
        }
    }
}

impl<'a> Highlighter for SyntectHighlighter<'a> {
    fn highlight<'b, L: Iterator<Item = &'b PagerLine>>(&self, lines: L) -> HighlightInfo {
        let mut info = HighlightInfo::none();

        let highlighter = highlighting::Highlighter::new(self.theme);
        let mut hstate = highlighting::HighlightState::new(&highlighter, ScopeStack::new());
        let mut parse_state = self.base_state.clone();

        for line in lines {
            let line_content = line.get_content();
            let mut current_pos = 0;
            let mut this_line_changes = Vec::new();

            let ops = parse_state.parse_line(line.get_content());
            for (style, fragment) in highlighting::HighlightIterator::new(
                &mut hstate,
                &ops[..],
                line_content,
                &highlighter,
            ) {
                this_line_changes.push((current_pos, to_unsegen_style_modifier(&style)));
                current_pos += fragment.len();
            }
            info.style_changes.push(this_line_changes);
        }
        info.default_style = to_unsegen_style_modifier(&highlighter.get_default());
        info
    }
}

fn to_unsegen_color(color: &highlighting::Color) -> Color {
    Color::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}
fn to_unsegen_text_format(style: &highlighting::FontStyle) -> TextFormatModifier {
    TextFormatModifier {
        bold: style.contains(highlighting::FontStyle::BOLD).into(),
        italic: style.contains(highlighting::FontStyle::ITALIC).into(),
        invert: ModifyMode::LeaveUnchanged,
        underline: style.contains(highlighting::FontStyle::UNDERLINE).into(),
    }
}
fn to_unsegen_style_modifier(style: &highlighting::Style) -> StyleModifier {
    StyleModifier::new()
        .fg_color(to_unsegen_color(&style.foreground))
        .bg_color(to_unsegen_color(&style.background))
        .format(to_unsegen_text_format(&style.font_style))
}

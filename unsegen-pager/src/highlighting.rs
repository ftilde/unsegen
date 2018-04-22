use unsegen::base::{Color, ModifyMode, StyleModifier, TextFormatModifier};

use syntect::parsing::{ParseState, ScopeStack, SyntaxDefinition};
use syntect::highlighting;
use syntect::highlighting::Theme;

pub trait Highlighter {
    type Instance: HighlightingInstance;
    fn create_instance(&self) -> Self::Instance;
}

pub trait HighlightingInstance {
    fn highlight<'a>(
        &mut self,
        line: &'a str,
    ) -> Box<Iterator<Item = (StyleModifier, &'a str)> + 'a>;
    fn default_style(&self) -> StyleModifier;
}

pub struct NoHighlighter;

impl Highlighter for NoHighlighter {
    type Instance = NoopHighlightingInstance;
    fn create_instance(&self) -> Self::Instance {
        NoopHighlightingInstance
    }
}

pub struct NoopHighlightingInstance;

impl HighlightingInstance for NoopHighlightingInstance {
    fn highlight<'a>(
        &mut self,
        line: &'a str,
    ) -> Box<Iterator<Item = (StyleModifier, &'a str)> + 'a> {
        Box::new(Some((StyleModifier::none(), line)).into_iter())
    }
    fn default_style(&self) -> StyleModifier {
        StyleModifier::none()
    }
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
    type Instance = SyntectHighlightingInstance<'a>;
    fn create_instance(&self) -> Self::Instance {
        SyntectHighlightingInstance::new(self.base_state.clone(), self.theme)
    }
}

pub struct SyntectHighlightingInstance<'a> {
    highlighter: highlighting::Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: highlighting::HighlightState,
}

impl<'a> SyntectHighlightingInstance<'a> {
    fn new(base_state: ParseState, theme: &'a highlighting::Theme) -> Self {
        let highlighter = highlighting::Highlighter::new(theme);
        let hstate = highlighting::HighlightState::new(&highlighter, ScopeStack::new());
        SyntectHighlightingInstance {
            highlighter: highlighter,
            parse_state: base_state,
            highlight_state: hstate,
        }
    }
}

impl<'b> HighlightingInstance for SyntectHighlightingInstance<'b> {
    fn highlight<'a>(
        &mut self,
        line: &'a str,
    ) -> Box<Iterator<Item = (StyleModifier, &'a str)> + 'a> {
        let ops = self.parse_state.parse_line(line);
        let iter: Vec<(highlighting::Style, &'a str)> = highlighting::HighlightIterator::new(
            &mut self.highlight_state,
            &ops[..],
            line,
            &self.highlighter,
        ).collect();
        Box::new(
            iter.into_iter()
                .map(|(style, line)| (to_unsegen_style_modifier(&style), line)),
        )
    }
    fn default_style(&self) -> StyleModifier {
        to_unsegen_style_modifier(&self.highlighter.get_default())
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

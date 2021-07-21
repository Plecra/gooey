use approx::relative_eq;
use gooey_core::{
    euclid::{Length, Size2D},
    styles::Alignment,
    Points,
};
use gooey_renderer::{Renderer, TextMetrics};

mod measured;
mod tokenizer;
pub(crate) use self::{measured::*, tokenizer::*};
use crate::{
    prepared::{PreparedLine, PreparedSpan, PreparedText},
    Text,
};

pub(crate) struct TextWrapper<'a, R: Renderer> {
    options: TextWrap,
    scene: &'a R,
    prepared_text: PreparedText,
}

pub(crate) enum ParserStatus {
    LineStart,
    InWord,
    TrailingPunctuation,
    Whitespace,
}

struct TextWrapState {
    width: Option<Length<f32, Points>>,
    current_vmetrics: Option<TextMetrics<Points>>,
    current_span_offset: Length<f32, Points>,
    current_groups: Vec<SpanGroup>,
    lines: Vec<PreparedLine>,
}

impl TextWrapState {
    #[must_use]
    fn push_group(&mut self, group: SpanGroup, single_line: bool) -> bool {
        let mut new_line = false;
        if let SpanGroup::EndOfLine(metrics) = &group {
            self.update_vmetrics(*metrics);
            self.new_line();
            true
        } else {
            let spans = group.spans();
            let total_width = spans
                .iter()
                .map(|s| s.metrics().width)
                .fold(Length::default(), |sum, width| sum + width);

            if let Some(width) = self.width {
                let new_width = total_width + self.current_span_offset;
                let remaining_width = width - new_width;

                // If the value is negative and isn't zero (-0. is a valid float)
                if !relative_eq!(remaining_width.get(), 0., epsilon = 0.001)
                    && remaining_width.get().is_sign_negative()
                {
                    if relative_eq!(self.current_span_offset.get(), 0.) {
                        // TODO Split the group if it can't fit on a single line
                        // For now, just render it anyways.
                    } else {
                        self.new_line();
                        new_line = true;
                        if single_line {
                            return new_line;
                        }
                    }
                }
            }
            self.current_span_offset += total_width;
            self.current_groups.push(group);
            new_line
        }
    }

    fn update_vmetrics(&mut self, new_metrics: TextMetrics<Points>) {
        self.current_vmetrics = match self.current_vmetrics {
            Some(metrics) => Some(TextMetrics {
                ascent: metrics.ascent.max(new_metrics.ascent),
                descent: metrics.descent.min(new_metrics.descent),
                line_gap: metrics.line_gap.max(new_metrics.line_gap),
                width: Length::default(),
            }),
            None => Some(new_metrics),
        }
    }

    fn position_span(&mut self, span: &mut PreparedSpan) {
        let width = span.metrics().width;
        span.set_location(self.current_span_offset);
        self.current_span_offset += width;
    }

    fn new_line(&mut self) {
        // Remove any whitespace from the end of the line
        while matches!(self.current_groups.last(), Some(SpanGroup::Whitespace(_))) {
            self.current_groups.pop();
        }

        let mut spans = Vec::new();
        for group in &self.current_groups {
            for span in group.spans() {
                spans.push(span);
            }
        }

        self.current_span_offset = Length::default();
        for span in &mut spans {
            self.update_vmetrics(*span.metrics());
            self.position_span(span)
        }

        if let Some(metrics) = self.current_vmetrics.take() {
            self.lines.push(PreparedLine {
                spans,
                metrics,
                alignment_offset: Length::default(),
            });
        }
        self.current_span_offset = Length::default();
        self.current_groups.clear();
    }

    fn finish(mut self) -> Vec<PreparedLine> {
        if !self.current_groups.is_empty() || self.lines.is_empty() {
            self.new_line();
        }

        self.lines
    }
}

impl<'a, R: Renderer> TextWrapper<'a, R> {
    pub fn wrap(text: &Text, scene: &'a R, options: TextWrap) -> PreparedText {
        TextWrapper {
            options,
            scene,
            prepared_text: PreparedText::default(),
        }
        .wrap_text(text)
    }

    fn wrap_text(mut self, text: &Text) -> PreparedText {
        let width = self.options.width();

        let measured = MeasuredText::new(text, self.scene);

        let mut state = TextWrapState {
            width,
            current_span_offset: Length::default(),
            current_vmetrics: None,
            current_groups: Vec::new(),
            lines: Vec::new(),
        };

        match measured.info {
            MeasuredTextInfo::Groups(groups) => {
                let single_line = self.options.is_single_line();
                for group in groups {
                    if state.push_group(group, single_line) && single_line {
                        println!("stopping iteration over groups");
                        break;
                    }
                }

                self.prepared_text.lines = state.finish();
            }
            MeasuredTextInfo::NoText(metrics) => {
                self.prepared_text.lines.push(PreparedLine {
                    metrics,
                    alignment_offset: Length::default(),
                    spans: Vec::default(),
                });
            }
        }

        if let Some(alignment) = self.options.alignment() {
            if let Some(max_width) = self.options.width() {
                self.prepared_text.align(alignment, max_width);
            }
        }

        self.prepared_text
    }
}

/// Text wrapping options.
#[derive(Debug, Clone)]
pub enum TextWrap {
    /// Do not wrap the text.
    NoWrap,
    /// Render the text in a single line. Do not render past `max_width`.
    SingleLine {
        /// The width of the line to render within.
        width: Length<f32, Points>,
        /// If the text cannot fit within a single line, truncate the text.
        truncate: bool,
        /// Within `max_width`, use this `alignment`.
        alignment: Alignment,
    },
    /// Render a multi-line text block.
    MultiLine {
        /// The width of the text area.
        size: Size2D<f32, Points>,
        /// Controls the alignment of the lines of text.
        alignment: Alignment,
    },
}

impl TextWrap {
    /// Returns true if this is a multiline wrap.
    #[must_use]
    pub fn is_multiline(&self) -> bool {
        matches!(self, Self::MultiLine { .. })
    }

    /// Returns true if this is a single-line wrap.
    #[must_use]
    pub fn is_single_line(&self) -> bool {
        !self.is_multiline()
    }

    /// Returns the width of the rendered area, if one was provided.
    #[must_use]
    pub fn width(&self) -> Option<Length<f32, Points>> {
        match self {
            Self::MultiLine { size, .. } => Some(Length::new(size.width)),
            Self::SingleLine { width, .. } => Some(*width),
            Self::NoWrap => None,
        }
    }

    /// Returns the height of the rendendered area, if one was provided.
    #[must_use]
    pub fn height(&self) -> Option<Length<f32, Points>> {
        match self {
            Self::MultiLine { size, .. } => Some(Length::new(size.height)),
            _ => None,
        }
    }

    /// Returns whether to truncate the text or not when rendering.
    #[must_use]
    pub fn truncate(&self) -> bool {
        match self {
            Self::SingleLine { truncate, .. } => *truncate,
            _ => false,
        }
    }

    /// Returns the alignment to use.
    #[must_use]
    pub fn alignment(&self) -> Option<Alignment> {
        match self {
            Self::NoWrap => None,
            Self::MultiLine { alignment, .. } | Self::SingleLine { alignment, .. } =>
                Some(*alignment),
        }
    }
}

#[cfg(test)]
mod tests {
    use gooey_core::{
        styles::{FontSize, Style},
        Pixels,
    };

    use super::*;
    use crate::Span;

    #[derive(Debug)]
    struct MockTextRenderer;

    impl Renderer for MockTextRenderer {
        fn size(&self) -> gooey_core::euclid::Size2D<f32, Points> {
            unimplemented!()
        }

        fn clip_to(&self, _bounds: gooey_core::euclid::Rect<f32, Points>) -> Self {
            unimplemented!()
        }

        fn clip_bounds(&self) -> gooey_core::euclid::Rect<f32, Points> {
            unimplemented!()
        }

        fn scale(&self) -> gooey_core::euclid::Scale<f32, Points, Pixels> {
            unimplemented!()
        }

        fn render_text<
            F: gooey_core::styles::FallbackComponent<Value = gooey_core::styles::ColorPair>,
        >(
            &self,
            _text: &str,
            _baseline_origin: gooey_core::euclid::Point2D<f32, Points>,
            _style: &gooey_core::styles::Style,
        ) {
            unimplemented!()
        }

        #[allow(clippy::cast_precision_loss)]
        fn measure_text(
            &self,
            text: &str,
            style: &gooey_core::styles::Style,
        ) -> TextMetrics<Points> {
            // Return a fixed width per character, based on the font size.
            let font_size = style
                .get::<FontSize<Points>>()
                .map_or_else(|| Length::new(14.), |size| size.0);
            TextMetrics {
                width: font_size * text.len() as f32 * 0.6,
                ascent: font_size * 0.8,
                descent: -font_size * 0.2,
                line_gap: font_size * 0.1,
            }
        }

        fn stroke_rect(
            &self,
            _rect: &gooey_core::euclid::Rect<f32, Points>,
            _style: &gooey_core::styles::Style,
        ) {
            unimplemented!()
        }

        fn fill_rect<
            F: gooey_core::styles::FallbackComponent<Value = gooey_core::styles::ColorPair>,
        >(
            &self,
            _rect: &gooey_core::euclid::Rect<f32, Points>,
            _style: &gooey_core::styles::Style,
        ) {
            unimplemented!()
        }

        fn stroke_line(
            &self,
            _point_a: gooey_core::euclid::Point2D<f32, Points>,
            _point_b: gooey_core::euclid::Point2D<f32, Points>,
            _style: &gooey_core::styles::Style,
        ) {
            unimplemented!()
        }
    }

    #[test]
    /// This test should have "This line should " be on the first line and "wrap" on the second
    fn wrap_one_word() {
        let scene = MockTextRenderer;
        let wrap = Text::from(vec![Span::new(
            "This line should wrap",
            Style::new().with(FontSize::<Points>::new(12.)),
        )])
        .wrap(&scene, TextWrap::MultiLine {
            size: Size2D::new(80.0, f32::MAX),
            alignment: Alignment::Left,
        });
        println!("Wrapped text: {:#?}", wrap);
        assert_eq!(wrap.lines.len(), 2);
        assert_eq!(wrap.lines[0].spans.len(), 3); // "this"," ","line"
        assert_eq!(wrap.lines[1].spans.len(), 3); // "should"," ","wrap"
        assert_eq!(wrap.lines[1].spans[0].text(), "should");
    }

    #[test]
    /// This test should have "This line should " be on the first line and "wrap" on the second
    fn wrap_one_word_different_span() {
        let scene = MockTextRenderer;

        let first_style = Style::new().with(FontSize::<Points>::new(12.));

        let second_style = Style::new().with(FontSize::<Points>::new(10.));

        let wrap = Text::from(vec![
            Span::new("This line should ", first_style),
            Span::new("wrap", second_style),
        ])
        .wrap(&scene, TextWrap::MultiLine {
            size: Size2D::new(130.0, f32::MAX),
            alignment: Alignment::Left,
        });
        assert_eq!(wrap.lines.len(), 2);
        assert_eq!(wrap.lines[0].spans.len(), 5);
        assert_eq!(wrap.lines[1].spans.len(), 1);
        assert_eq!(wrap.lines[1].spans[0].text(), "wrap");
        assert_ne!(
            wrap.lines[0].spans[0].metrics().ascent,
            wrap.lines[1].spans[0].metrics().ascent
        );
    }
}

use gooey_core::{
    euclid::{Length, Size2D, Vector2D},
    renderer::Renderer,
    styles::{ForegroundColor, Points, Srgba, Style},
    Transmogrifier,
};
use gooey_rasterizer::{RasterContext, Rasterizer, WidgetRasterizer};

use crate::button::{Button, ButtonTransmogrifier};

const BUTTON_PADDING: Length<f32, Points> = Length::new(5.);

impl<R: Renderer> Transmogrifier<Rasterizer<R>> for ButtonTransmogrifier {
    type State = ();
    type Widget = Button;
}

impl<R: Renderer> WidgetRasterizer<R> for ButtonTransmogrifier {
    fn render(&self, context: RasterContext<Self, R>) {
        if let Some(scene) = context.rasterizer.renderer() {
            scene.fill_rect(
                &scene.bounds(),
                &Style::new().with(ForegroundColor(Srgba::new(0., 1., 0., 1.).into())),
            );

            let text_size = scene.measure_text(&context.widget.label, &Style::default());

            let center = scene.bounds().center();
            scene.render_text(
                &context.widget.label,
                center - Vector2D::from_lengths(text_size.width, text_size.height()) / 2.
                    + Vector2D::from_lengths(Length::default(), text_size.ascent),
                &Style::default(),
            );
        }
    }

    fn content_size(
        &self,
        context: RasterContext<Self, R>,
        _constraints: Size2D<Option<f32>, Points>,
    ) -> Size2D<f32, Points> {
        if let Some(scene) = context.rasterizer.renderer() {
            // TODO should be wrapped width
            let text_size = scene.measure_text(&context.widget.label, &Style::default());
            (Vector2D::from_lengths(text_size.width, text_size.height())
                + Vector2D::from_lengths(BUTTON_PADDING * 2., BUTTON_PADDING * 2.))
            .to_size()
        } else {
            Size2D::default()
        }
    }
}

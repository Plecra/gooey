use gooey_core::{
    euclid::{Point2D, Rect, Size2D},
    Points, Transmogrifier, TransmogrifierContext,
};
use gooey_rasterizer::{ContentArea, Rasterizer, Renderer, WidgetRasterizer};

use crate::container::{Container, ContainerTransmogrifier};

impl<R: Renderer> Transmogrifier<Rasterizer<R>> for ContainerTransmogrifier {
    type State = ();
    type Widget = Container;
}

impl<R: Renderer> WidgetRasterizer<R> for ContainerTransmogrifier {
    fn render(
        &self,
        context: &mut TransmogrifierContext<'_, Self, Rasterizer<R>>,
        content_area: &ContentArea,
    ) {
        context.frontend.with_transmogrifier(
            context.widget.child.id(),
            |child_transmogrifier, mut child_context| {
                let child_content_area = child_transmogrifier
                    .content_size(
                        &mut child_context,
                        Size2D::new(
                            Some(content_area.size.content.width),
                            Some(content_area.size.content.height),
                        ),
                    )
                    .total_size();
                let remaining_size = content_area.size.content - child_content_area;

                // TODO respect Alignment + Vertical alignment
                let child_rect = Rect::new(
                    Point2D::new(remaining_size.width / 2., remaining_size.height / 2.),
                    child_content_area,
                );
                child_transmogrifier.render_within(&mut child_context, child_rect, context.style);
            },
        );
    }

    fn measure_content(
        &self,
        context: &mut TransmogrifierContext<'_, Self, Rasterizer<R>>,
        constraints: Size2D<Option<f32>, Points>,
    ) -> Size2D<f32, Points> {
        context
            .frontend
            .with_transmogrifier(
                context.widget.child.id(),
                |child_transmogrifier, mut child_context| {
                    child_transmogrifier
                        .content_size(&mut child_context, constraints)
                        .content
                },
            )
            .unwrap_or_default()
    }
}

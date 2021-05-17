use gooey_widgets::button::{Button, ButtonTransmogrifier};

use crate::{window_document, WebSys, WebSysTransmogrifier};

impl gooey_core::Transmogrifier<WebSys> for ButtonTransmogrifier {
    type Widget = Button;
    type Context = web_sys::Node;

    fn content_size(
        &self,
        widget: &Self::Widget,
        constraints: gooey_core::euclid::Size2D<Option<f32>, gooey_core::stylecs::Points>,
        context: &Self::Context,
    ) -> gooey_core::euclid::Size2D<f32, gooey_core::stylecs::Points> {
        todo!()
    }
}

impl WebSysTransmogrifier for ButtonTransmogrifier {
    fn transmogrify(&self, parent: &web_sys::Node, widget: &Button) -> Option<web_sys::Element> {
        let document = window_document();
        let element = document
            .create_element("button")
            .expect("couldn't create button");
        // TODO escape html entities
        element.set_inner_html(&widget.label);
        parent.append_child(&element).unwrap();
        Some(element)
    }
}

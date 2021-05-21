use std::{any::TypeId, sync::Arc};

use gooey_core::{
    AnyChannels, AnySendSync, AnyTransmogrifier, AnyWidget, Channels, Gooey, Transmogrifier,
    TransmogrifierState, Widget, WidgetRef, WidgetRegistration, WidgetStorage,
};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
pub struct WebSys {
    pub ui: Gooey<Self>,
}

impl WebSys {
    pub fn new(ui: Gooey<Self>) -> Self {
        Self { ui }
    }

    pub fn install_in_id(&self, id: &str) {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let parent = document.get_element_by_id(id).expect("id not found");

        self.ui.with_transmogrifier(
            self.ui.root_widget().id(),
            |transmogrifier, state, widget, channels| {
                transmogrifier.transmogrify(state, channels, &parent, widget, self);
            },
        );
    }
}

#[derive(Debug)]
pub struct RegisteredTransmogrifier(pub Box<dyn AnyWidgetWebSysTransmogrifier>);

impl AnyWidgetWebSysTransmogrifier for RegisteredTransmogrifier {
    fn transmogrify(
        &self,
        state: &mut dyn AnySendSync,
        channels: &dyn AnyChannels,
        parent: &web_sys::Node,
        widget: &dyn AnyWidget,
        gooey: &WebSys,
    ) -> Option<web_sys::HtmlElement> {
        self.0.transmogrify(state, channels, parent, widget, gooey)
    }
}

impl gooey_core::Frontend for WebSys {
    type AnyTransmogrifier = RegisteredTransmogrifier;
    type Context = WebSys;

    fn gooey(&self) -> &'_ Gooey<Self> {
        &self.ui
    }
}

pub trait WebSysTransmogrifier: Transmogrifier<WebSys> {
    fn transmogrify(
        &self,
        state: &Self::State,
        channels: &Channels<<Self as Transmogrifier<WebSys>>::Widget>,
        parent: &web_sys::Node,
        widget: &<Self as Transmogrifier<WebSys>>::Widget,
        gooey: &WebSys,
    ) -> Option<web_sys::HtmlElement>;
}

pub trait AnyWidgetWebSysTransmogrifier: AnyTransmogrifier<WebSys> {
    fn transmogrify(
        &self,
        state: &mut dyn AnySendSync,
        channels: &dyn AnyChannels,
        parent: &web_sys::Node,
        widget: &dyn AnyWidget,
        gooey: &WebSys,
    ) -> Option<web_sys::HtmlElement>;
}

impl<T> AnyWidgetWebSysTransmogrifier for T
where
    T: WebSysTransmogrifier + AnyTransmogrifier<WebSys> + Send + Sync + 'static,
{
    fn transmogrify(
        &self,
        state: &mut dyn AnySendSync,
        channels: &dyn AnyChannels,
        parent: &web_sys::Node,
        widget: &dyn AnyWidget,
        gooey: &WebSys,
    ) -> Option<web_sys::HtmlElement> {
        let widget = widget
            .as_any()
            .downcast_ref::<<T as Transmogrifier<WebSys>>::Widget>()
            .unwrap();
        let state = state
            .as_mut_any()
            .downcast_mut::<<T as Transmogrifier<WebSys>>::State>()
            .unwrap();
        let channels = channels
            .as_any()
            .downcast_ref::<Channels<<T as Transmogrifier<WebSys>>::Widget>>()
            .unwrap();
        <T as WebSysTransmogrifier>::transmogrify(&self, state, channels, parent, widget, gooey)
    }
}

impl AnyTransmogrifier<WebSys> for RegisteredTransmogrifier {
    fn process_messages(
        &self,
        state: &mut dyn AnySendSync,
        widget: &mut dyn AnyWidget,
        channels: &dyn AnyChannels,
        storage: &WidgetStorage,
    ) {
        self.0.process_messages(state, widget, channels, storage)
    }

    fn widget_type_id(&self) -> TypeId {
        self.0.widget_type_id()
    }

    fn default_state_for(&self, widget: &Arc<WidgetRegistration>) -> TransmogrifierState {
        self.0.default_state_for(widget)
    }
}

#[macro_export]
macro_rules! make_browser {
    ($transmogrifier:ident) => {
        impl From<$transmogrifier> for $crate::RegisteredTransmogrifier {
            fn from(transmogrifier: $transmogrifier) -> Self {
                Self(std::boxed::Box::new(transmogrifier))
            }
        }
    };
}

pub struct WidgetClosure;

impl WidgetClosure {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<W: Widget, C: FnMut() -> <W as Widget>::TransmogrifierEvent + 'static>(
        widget: WidgetRef<W, WebSys>,
        mut event_generator: C,
    ) -> Closure<dyn FnMut()> {
        Closure::wrap(Box::new(move || {
            widget.post_event(event_generator());
        }) as Box<dyn FnMut()>)
    }
}

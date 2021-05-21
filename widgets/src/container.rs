use std::sync::Arc;

use gooey_core::{
    euclid::Length,
    styles::{Points, Surround},
    Widget, WidgetRegistration, WidgetStorage,
};

#[cfg(feature = "gooey-rasterizer")]
mod rasterizer;

#[cfg(feature = "frontend-browser")]
mod browser;

#[derive(Debug)]
pub struct Container {
    pub child: Arc<WidgetRegistration>,
    pub padding: Surround<Points>,
}

impl Container {
    pub fn new<W: Widget>(child: W, storage: &WidgetStorage) -> Self {
        Self {
            child: storage.register(child),
            padding: Surround::default(),
        }
    }

    pub fn pad_left<F: Into<Length<f32, Points>>>(mut self, padding: F) -> Self {
        self.padding.left = Some(padding.into().get());
        self
    }

    pub fn pad_right<F: Into<Length<f32, Points>>>(mut self, padding: F) -> Self {
        self.padding.right = Some(padding.into().get());
        self
    }

    pub fn pad_top<F: Into<Length<f32, Points>>>(mut self, padding: F) -> Self {
        self.padding.top = Some(padding.into().get());
        self
    }

    pub fn pad_bottom<F: Into<Length<f32, Points>>>(mut self, padding: F) -> Self {
        self.padding.bottom = Some(padding.into().get());
        self
    }
}

impl Widget for Container {
    type TransmogrifierCommand = ();
    type TransmogrifierEvent = ();
}

#[derive(Debug)]
pub struct ContainerTransmogrifier;

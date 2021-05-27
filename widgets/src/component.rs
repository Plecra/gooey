use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
    sync::{Arc, RwLock},
};

use gooey_core::{
    AnyWidget, Callback, CallbackFn, Channels, Context, Frontend, Transmogrifier,
    WeakWidgetRegistration, Widget, WidgetId, WidgetRef, WidgetRegistration, WidgetStorage,
};

#[cfg(feature = "gooey-rasterizer")]
mod rasterizer;

#[cfg(feature = "frontend-browser")]
mod browser;

#[derive(Debug)]
pub struct Component<B: Behavior> {
    pub behavior: B,
    content: WidgetRegistration,
    content_widget: Option<WidgetRef<B::Content>>,
    registered_widgets: HashMap<B::Widgets, WeakWidgetRegistration>,
    callback_widget: SettableWidgetRef<B>,
}

impl<B: Behavior> Component<B> {
    pub fn new(storage: &WidgetStorage) -> Self {
        B::initialize(CallbackMapper::new(storage))
    }

    pub fn initialized(widget: B::Content, behavior: B, callbacks: CallbackMapper<B>) -> Self {
        let content = callbacks.register(widget);
        Self {
            behavior,
            content,
            content_widget: None,
            callback_widget: callbacks.widget,
            registered_widgets: callbacks.registered_widgets,
        }
    }

    pub fn content(&self) -> Option<&'_ WidgetRef<B::Content>> {
        self.content_widget.as_ref()
    }

    pub fn registered_widget(&self, id: &B::Widgets) -> Option<WidgetRegistration> {
        self.registered_widgets.get(id).and_then(|id| id.upgrade())
    }
}

#[derive(Debug)]
pub struct ComponentTransmogrifier<B: Behavior>(PhantomData<B>);

impl<B: Behavior> Default for ComponentTransmogrifier<B> {
    fn default() -> Self {
        Self(PhantomData::default())
    }
}

pub trait Behavior: Debug + Send + Sync + Sized + 'static {
    type Event: Debug + Send + Sync;
    type Content: Widget;
    type Widgets: Hash + Eq + Debug + Send + Sync;

    fn initialize(callbacks: CallbackMapper<Self>) -> Component<Self>;

    fn receive_event(
        component: &mut Component<Self>,
        event: Self::Event,
        context: &Context<Component<Self>>,
    );
}

impl<B: Behavior> Widget for Component<B> {
    type Command = <B::Content as Widget>::Command;
    type TransmogrifierCommand = <B::Content as Widget>::Command;
    type TransmogrifierEvent = InternalEvent<B>;

    fn receive_event(&mut self, event: Self::TransmogrifierEvent, context: &Context<Self>) {
        log::info!("Component Widget received event: {:?}", event);
        match event {
            InternalEvent::ReceiveWidget(widget) => {
                self.content_widget = Some(widget);
            }
            InternalEvent::Content(event) => B::receive_event(self, event, context),
        }
    }

    fn receive_command(&mut self, command: Self::Command, context: &Context<Self>) {
        log::info!("Component Widget received command: {:?}", command);
        context.send_command(command);
    }
}

#[derive(Debug)]
pub enum InternalEvent<B: Behavior> {
    ReceiveWidget(WidgetRef<B::Content>),
    Content(B::Event),
}

#[derive(Debug)]
pub struct CallbackMapper<B: Behavior> {
    widget: SettableWidgetRef<B>,
    storage: WidgetStorage,
    registered_widgets: HashMap<B::Widgets, WeakWidgetRegistration>,
    _phantom: PhantomData<B>,
}

impl<B: Behavior> CallbackMapper<B> {
    pub fn new(storage: &WidgetStorage) -> Self {
        Self {
            storage: storage.clone(),
            widget: SettableWidgetRef::default(),
            registered_widgets: HashMap::default(),
            _phantom: PhantomData::default(),
        }
    }

    pub fn map_event<I: 'static, C: CallbackFn<I, <B as Behavior>::Event> + 'static>(
        &self,
        mapper: C,
    ) -> Callback<I, ()> {
        let mapped_callback = MappedCallback::<B, I> {
            mapper: Box::new(mapper),
            widget: self.widget.clone(),
            _phantom: PhantomData::default(),
        };
        Callback::new(mapped_callback)
    }

    /// Register a widget with storage.
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // The unwrap is unreachable
    pub fn register_with_id<W: Widget + AnyWidget>(
        &mut self,
        id: B::Widgets,
        widget: W,
    ) -> WidgetRegistration {
        let registration = self.storage.register(widget);
        self.registered_widgets
            .insert(id, WeakWidgetRegistration::from(&registration));
        registration
    }
}

impl<B: Behavior> Deref for CallbackMapper<B> {
    type Target = WidgetStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

pub struct MappedCallback<B: Behavior, I> {
    widget: SettableWidgetRef<B>,
    mapper: Box<dyn CallbackFn<I, <B as Behavior>::Event>>,
    _phantom: PhantomData<B>,
}

type SettableWidgetRef<B> = Arc<RwLock<Option<Box<dyn AnyEventPoster<B>>>>>;

#[derive(Debug)]
pub struct EventPoster<B: Behavior, F: Frontend> {
    widget: WidgetId,
    channels: Channels<Component<B>>,
    frontend: F,
}

impl<B: Behavior, F: Frontend> AnyEventPoster<B> for EventPoster<B, F> {
    fn post_event(&self, event: B::Event) {
        log::info!("AnyEventPoster posting: {:?}", event);
        let _ = self.channels.post_event(InternalEvent::Content(event));
        self.frontend
            .gooey()
            .set_widget_has_messages(self.widget.clone());
        self.frontend
            .gooey()
            .process_widget_messages(&self.frontend);
    }
}

pub trait AnyEventPoster<B: Behavior>: Debug + Send + Sync + 'static {
    fn post_event(&self, event: B::Event);
}

impl<B: Behavior, I> Debug for MappedCallback<B, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "MappedCallback {{ widget: {:?} }}",
            &self.widget
        ))
    }
}

impl<B: Behavior, I> CallbackFn<I, ()> for MappedCallback<B, I> {
    fn invoke(&self, info: I) {
        log::info!("Invoking mapped callback");
        let poster = self.widget.read().unwrap();
        let poster = poster.as_ref().unwrap();
        poster.post_event(self.mapper.invoke(info));
    }
}

impl<B: Behavior, F: Frontend + Send + Sync> Transmogrifier<F> for ComponentTransmogrifier<B> {
    type State = ();
    type Widget = Component<B>;

    fn initialize(
        &self,
        component: &Self::Widget,
        widget: &WidgetRef<Self::Widget>,
        frontend: &F,
    ) -> Self::State {
        let widget = widget.registration().unwrap().id().clone();
        let widget_state = frontend.gooey().widget_state(widget.id).unwrap();
        let channels = widget_state.channels::<Self::Widget>().unwrap();
        let mut callback_widget = component.callback_widget.write().unwrap();
        *callback_widget = Some(Box::new(EventPoster {
            widget,
            channels: channels.clone(),
            frontend: frontend.clone(),
        }));
        channels.post_event(InternalEvent::ReceiveWidget(
            WidgetRef::new(&component.content, frontend.clone()).expect(
                "type mismatch: Behavior::Widget type doesn't match initialized widget type",
            ),
        ));
    }

    fn receive_command(
        &self,
        _state: &mut Self::State,
        command: <Self::Widget as Widget>::TransmogrifierCommand,
        widget: &Self::Widget,
        _frontend: &F,
    ) {
        log::info!("Component Transmogrifier received command: {:?}", command);
        widget
            .content_widget
            .as_ref()
            .unwrap()
            .post_command::<F>(command);
    }
}

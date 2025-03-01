//! Types for displaying a [`Widget`] inside of a desktop window.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::ops::{Deref, DerefMut, Not};
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::path::Path;
use std::string::ToString;
use std::sync::{MutexGuard, OnceLock};

use ahash::AHashMap;
use alot::LotId;
use arboard::Clipboard;
use kludgine::app::winit::dpi::{PhysicalPosition, PhysicalSize};
use kludgine::app::winit::event::{
    DeviceId, ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase,
};
use kludgine::app::winit::keyboard::{Key, NamedKey};
use kludgine::app::winit::window;
use kludgine::app::WindowBehavior as _;
use kludgine::cosmic_text::{Family, FamilyOwned};
use kludgine::figures::units::{Px, UPx};
use kludgine::figures::{IntoSigned, IntoUnsigned, Point, Ranged, Rect, ScreenScale, Size, Zero};
use kludgine::render::Drawing;
use kludgine::wgpu::CompositeAlphaMode;
use kludgine::Kludgine;
use tracing::Level;

use crate::animation::{LinearInterpolate, PercentBetween, ZeroToOne};
use crate::app::Gooey;
use crate::context::{
    AsEventContext, EventContext, Exclusive, GraphicsContext, InvalidationStatus, LayoutContext,
    WidgetContext,
};
use crate::graphics::{FontState, Graphics};
use crate::styles::{Edges, FontFamilyList, ThemePair};
use crate::tree::Tree;
use crate::utils::ModifiersExt;
use crate::value::{Dynamic, DynamicReader, IntoDynamic, IntoValue, Value};
use crate::widget::{
    EventHandling, MountedWidget, RootBehavior, Widget, WidgetId, WidgetInstance, HANDLED, IGNORED,
};
use crate::window::sealed::WindowCommand;
use crate::{initialize_tracing, ConstraintLimit, Run};

/// A currently running Gooey window.
pub struct RunningWindow<'window> {
    window: kludgine::app::Window<'window, WindowCommand>,
    gooey: Gooey,
    focused: Dynamic<bool>,
    occluded: Dynamic<bool>,
}

impl<'window> RunningWindow<'window> {
    pub(crate) fn new(
        window: kludgine::app::Window<'window, WindowCommand>,
        gooey: &Gooey,
        focused: &Dynamic<bool>,
        occluded: &Dynamic<bool>,
    ) -> Self {
        Self {
            window,
            gooey: gooey.clone(),
            focused: focused.clone(),
            occluded: occluded.clone(),
        }
    }

    /// Returns a dynamic that is updated whenever this window's focus status
    /// changes.
    #[must_use]
    pub const fn focused(&self) -> &Dynamic<bool> {
        &self.focused
    }

    /// Returns a dynamic that is updated whenever this window's occlusion
    /// status changes.
    #[must_use]
    pub fn occluded(&self) -> &Dynamic<bool> {
        &self.occluded
    }

    /// Returns a locked mutex guard to the OS's clipboard, if one was able to be
    /// initialized when the window opened.
    #[must_use]
    pub fn clipboard_guard(&self) -> Option<MutexGuard<'_, Clipboard>> {
        self.gooey.clipboard_guard()
    }
}

impl<'window> Deref for RunningWindow<'window> {
    type Target = kludgine::app::Window<'window, WindowCommand>;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl<'window> DerefMut for RunningWindow<'window> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window
    }
}

/// The attributes of a Gooey window.
pub type WindowAttributes = kludgine::app::WindowAttributes;

/// A Gooey window that is not yet running.
#[must_use]
pub struct Window<Behavior>
where
    Behavior: WindowBehavior,
{
    context: Behavior::Context,
    gooey: Gooey,
    /// The attributes of this window.
    pub attributes: WindowAttributes,
    /// The colors to use to theme the user interface.
    pub theme: Value<ThemePair>,
    /// When true, the system fonts will be loaded into the font database. This
    /// is on by default.
    pub load_system_fonts: bool,
    /// The list of font families to try to find when a [`FamilyOwned::Serif`]
    /// font is requested.
    pub serif_font_family: FontFamilyList,
    /// The list of font families to try to find when a
    /// [`FamilyOwned::SansSerif`] font is requested.
    pub sans_serif_font_family: FontFamilyList,
    /// The list of font families to try to find when a [`FamilyOwned::Fantasy`]
    /// font is requested.
    pub fantasy_font_family: FontFamilyList,
    /// The list of font families to try to find when a
    /// [`FamilyOwned::Monospace`] font is requested.
    pub monospace_font_family: FontFamilyList,
    /// The list of font families to try to find when a [`FamilyOwned::Cursive`]
    /// font is requested.
    pub cursive_font_family: FontFamilyList,
    /// A list of data buffers that contain font data to ensure are loaded
    /// during drawing operations.
    pub font_data_to_load: Vec<Vec<u8>>,

    occluded: Option<Dynamic<bool>>,
    focused: Option<Dynamic<bool>>,
    theme_mode: Option<Value<ThemeMode>>,
}

impl<Behavior> Default for Window<Behavior>
where
    Behavior: WindowBehavior,
    Behavior::Context: Default,
{
    fn default() -> Self {
        let context = Behavior::Context::default();
        Self::new(context, Gooey::default())
    }
}

impl Window<WidgetInstance> {
    /// Returns a new instance using `widget` as its contents.
    pub fn for_widget<W>(widget: W) -> Self
    where
        W: Widget,
    {
        Self::new(WidgetInstance::new(widget), Gooey::default())
    }

    /// Sets `focused` to be the dynamic updated when this window's focus status
    /// is changed.
    ///
    /// When the window is focused for user input, the dynamic will contain
    /// `true`.
    ///
    /// `focused` will be initialized with an initial state
    /// of `false`.
    pub fn focused(mut self, focused: impl IntoDynamic<bool>) -> Self {
        let focused = focused.into_dynamic();
        focused.set(false);
        self.focused = Some(focused);
        self
    }

    /// Sets `occluded` to be the dynamic updated when this window's occlusion
    /// status is changed.
    ///
    /// When the window is occluded (completely hidden/offscreen/minimized), the
    /// dynamic will contain `true`. If the window is at least partially
    /// visible, this value will contain `true`.
    ///
    /// `occluded` will be initialized with an initial state of `false`.
    pub fn occluded(mut self, occluded: impl IntoDynamic<bool>) -> Self {
        let occluded = occluded.into_dynamic();
        occluded.set(false);
        self.occluded = Some(occluded);
        self
    }

    /// Sets the [`ThemeMode`] for this window.
    ///
    /// If a [`ThemeMode`] is provided, the window will be set to this theme
    /// mode upon creation and will not be updated while the window is running.
    ///
    /// If a [`Dynamic`] is provided, the initial value will be ignored and the
    /// dynamic will be updated when the window opens with the user's current
    /// theme mode. The dynamic will also be updated any time the user's theme
    /// mode changes.
    ///
    /// Setting the [`Dynamic`]'s value will also update the window with the new
    /// mode until a mode change is detected, upon which the new mode will be
    /// stored.
    pub fn themed_mode(mut self, theme_mode: impl IntoValue<ThemeMode>) -> Self {
        self.theme_mode = Some(theme_mode.into_value());
        self
    }

    /// Applies `theme` to the widgets in this window.
    pub fn themed(mut self, theme: impl IntoValue<ThemePair>) -> Self {
        self.theme = theme.into_value();
        self
    }

    /// Adds `font_data` to the list of fonts to load for availability when
    /// rendering.
    ///
    /// All font families contained in `font_data` will be loaded.
    pub fn loading_font(mut self, font_data: Vec<u8>) -> Self {
        self.font_data_to_load.push(font_data);
        self
    }
}

impl<Behavior> Window<Behavior>
where
    Behavior: WindowBehavior,
{
    /// Returns a new instance using `context` to initialize the window upon
    /// opening.
    pub fn new(context: Behavior::Context, gooey: Gooey) -> Self {
        static EXECUTABLE_NAME: OnceLock<String> = OnceLock::new();

        let title = EXECUTABLE_NAME
            .get_or_init(|| {
                std::env::args_os()
                    .next()
                    .and_then(|path| {
                        Path::new(&path)
                            .file_name()
                            .and_then(OsStr::to_str)
                            .map(ToString::to_string)
                    })
                    .unwrap_or_else(|| String::from("Gooey App"))
            })
            .clone();
        Self {
            attributes: WindowAttributes {
                title,
                ..WindowAttributes::default()
            },
            gooey,
            context,
            load_system_fonts: true,
            theme: Value::default(),
            occluded: None,
            focused: None,
            theme_mode: None,
            serif_font_family: FontFamilyList::default(),
            sans_serif_font_family: FontFamilyList::default(),
            fantasy_font_family: FontFamilyList::default(),
            monospace_font_family: FontFamilyList::default(),
            cursive_font_family: FontFamilyList::default(),
            font_data_to_load: vec![
                #[cfg(feature = "roboto-flex")]
                include_bytes!("../assets/RobotoFlex.ttf").to_vec(),
            ],
        }
    }
}

impl<Behavior> Run for Window<Behavior>
where
    Behavior: WindowBehavior,
{
    fn run(self) -> crate::Result {
        initialize_tracing();
        GooeyWindow::<Behavior>::run_with(AssertUnwindSafe(sealed::Context {
            user: self.context,
            settings: RefCell::new(sealed::WindowSettings {
                gooey: self.gooey,
                transparent: self.attributes.transparent,
                attributes: Some(self.attributes),
                occluded: self.occluded,
                focused: self.focused,
                theme: Some(self.theme),
                theme_mode: self.theme_mode,
                font_data_to_load: self.font_data_to_load,
                serif_font_family: self.serif_font_family,
                sans_serif_font_family: self.sans_serif_font_family,
                fantasy_font_family: self.fantasy_font_family,
                monospace_font_family: self.monospace_font_family,
                cursive_font_family: self.cursive_font_family,
            }),
        }))
    }
}

/// The behavior of a Gooey window.
pub trait WindowBehavior: Sized + 'static {
    /// The type that is provided when initializing this window.
    type Context: UnwindSafe + Send + 'static;

    /// Return a new instance of this behavior using `context`.
    fn initialize(window: &mut RunningWindow<'_>, context: Self::Context) -> Self;

    /// Create the window's root widget. This function is only invoked once.
    fn make_root(&mut self) -> WidgetInstance;

    /// The window has been requested to close. If this function returns true,
    /// the window will be closed. Returning false prevents the window from
    /// closing.
    #[allow(unused_variables)]
    fn close_requested(&self, window: &mut RunningWindow<'_>) -> bool {
        true
    }

    /// Runs this behavior as an application.
    fn run() -> crate::Result
    where
        Self::Context: Default,
    {
        Self::run_with(<Self::Context>::default())
    }

    /// Runs this behavior as an application, initialized with `context`.
    fn run_with(context: Self::Context) -> crate::Result {
        Window::<Self>::new(context, Gooey::default()).run()
    }
}

struct GooeyWindow<T> {
    behavior: T,
    root: MountedWidget,
    contents: Drawing,
    should_close: bool,
    cursor: CursorState,
    mouse_buttons: AHashMap<DeviceId, AHashMap<MouseButton, WidgetId>>,
    redraw_status: InvalidationStatus,
    initial_frame: bool,
    occluded: Dynamic<bool>,
    focused: Dynamic<bool>,
    keyboard_activated: Option<WidgetId>,
    min_inner_size: Option<Size<UPx>>,
    max_inner_size: Option<Size<UPx>>,
    theme: Option<DynamicReader<ThemePair>>,
    current_theme: ThemePair,
    theme_mode: Value<ThemeMode>,
    transparent: bool,
    fonts: FontState,
    gooey: Gooey,
}

impl<T> GooeyWindow<T>
where
    T: WindowBehavior,
{
    fn request_close(
        should_close: &mut bool,
        behavior: &mut T,
        window: &mut RunningWindow<'_>,
    ) -> bool {
        *should_close |= behavior.close_requested(window);

        *should_close
    }

    fn keyboard_activate_widget(
        &mut self,
        is_pressed: bool,
        widget: Option<LotId>,
        window: &mut RunningWindow<'_>,
        kludgine: &mut Kludgine,
    ) {
        if is_pressed {
            if let Some(default) = widget.and_then(|id| self.root.tree.widget_from_node(id)) {
                if let Some(previously_active) = self
                    .keyboard_activated
                    .take()
                    .and_then(|id| self.root.tree.widget(id))
                {
                    EventContext::new(
                        WidgetContext::new(
                            previously_active,
                            &self.redraw_status,
                            &self.current_theme,
                            window,
                            self.theme_mode.get(),
                            &mut self.cursor,
                        ),
                        kludgine,
                    )
                    .deactivate();
                }
                EventContext::new(
                    WidgetContext::new(
                        default.clone(),
                        &self.redraw_status,
                        &self.current_theme,
                        window,
                        self.theme_mode.get(),
                        &mut self.cursor,
                    ),
                    kludgine,
                )
                .activate();
                self.keyboard_activated = Some(default.id());
            }
        } else if let Some(keyboard_activated) = self
            .keyboard_activated
            .take()
            .and_then(|id| self.root.tree.widget(id))
        {
            EventContext::new(
                WidgetContext::new(
                    keyboard_activated,
                    &self.redraw_status,
                    &self.current_theme,
                    window,
                    self.theme_mode.get(),
                    &mut self.cursor,
                ),
                kludgine,
            )
            .deactivate();
        }
    }

    fn constrain_window_resizing(
        &mut self,
        resizable: bool,
        window: &mut RunningWindow<'_>,
        graphics: &mut kludgine::Graphics<'_>,
    ) -> RootMode {
        let mut root_or_child = self.root.widget.clone();
        let mut root_mode = None;
        let mut padding = Edges::<Px>::default();

        loop {
            let Some(managed) = self.root.tree.widget(root_or_child.id()) else {
                break;
            };

            let mut context = EventContext::new(
                WidgetContext::new(
                    managed,
                    &self.redraw_status,
                    &self.current_theme,
                    window,
                    self.theme_mode.get(),
                    &mut self.cursor,
                ),
                graphics,
            );
            let mut widget = root_or_child.lock();
            match widget.as_widget().root_behavior(&mut context) {
                Some((behavior, child)) => {
                    let child = child.clone();
                    match behavior {
                        RootBehavior::PassThrough => {}
                        RootBehavior::Expand => {
                            root_mode = root_mode.or(Some(RootMode::Expand));
                        }
                        RootBehavior::Align => {
                            root_mode = root_mode.or(Some(RootMode::Align));
                        }
                        RootBehavior::Pad(edges) => {
                            padding += edges.into_px(context.kludgine.scale());
                        }
                        RootBehavior::Resize(range) => {
                            let padding = padding.size();
                            let min_width = range
                                .width
                                .minimum()
                                .map_or(Px::ZERO, |width| width.into_px(context.kludgine.scale()))
                                .saturating_add(padding.width);
                            let max_width = range
                                .width
                                .maximum()
                                .map_or(Px::MAX, |width| width.into_px(context.kludgine.scale()))
                                .saturating_add(padding.width);
                            let min_height = range
                                .height
                                .minimum()
                                .map_or(Px::ZERO, |height| height.into_px(context.kludgine.scale()))
                                .saturating_add(padding.height);
                            let max_height = range
                                .height
                                .maximum()
                                .map_or(Px::MAX, |height| height.into_px(context.kludgine.scale()))
                                .saturating_add(padding.height);

                            let new_min_size = (min_width > 0 || min_height > 0)
                                .then_some(Size::new(min_width, min_height).into_unsigned());

                            if new_min_size != self.min_inner_size && resizable {
                                context.set_min_inner_size(new_min_size);
                                self.min_inner_size = new_min_size;
                            }
                            let new_max_size = (max_width > 0 || max_height > 0)
                                .then_some(Size::new(max_width, max_height).into_unsigned());

                            if new_max_size != self.max_inner_size && resizable {
                                context.set_max_inner_size(new_max_size);
                            }
                            self.max_inner_size = new_max_size;

                            break;
                        }
                    }
                    drop(widget);

                    root_or_child = child.clone();
                }
                None => break,
            }
        }

        root_mode.unwrap_or(RootMode::Fit)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum RootMode {
    Fit,
    Expand,
    Align,
}

impl<T> kludgine::app::WindowBehavior<WindowCommand> for GooeyWindow<T>
where
    T: WindowBehavior,
{
    type Context = AssertUnwindSafe<sealed::Context<T::Context>>;

    fn initialize(
        window: kludgine::app::Window<'_, WindowCommand>,
        graphics: &mut kludgine::Graphics<'_>,
        AssertUnwindSafe(context): Self::Context,
    ) -> Self {
        let mut settings = context.settings.borrow_mut();
        let gooey = settings.gooey.clone();
        let occluded = settings.occluded.take().unwrap_or_default();
        let focused = settings.focused.take().unwrap_or_default();
        let theme = settings.theme.take().expect("theme always present");

        let fontdb = graphics.font_system().db_mut();
        for font_to_load in settings.font_data_to_load.drain(..) {
            fontdb.load_font_data(font_to_load);
        }

        let fonts = FontState::new(fontdb);

        if let Some(FamilyOwned::Name(name)) = fonts
            .find_available_font_family(&settings.serif_font_family)
            .or_else(|| default_family(Family::Serif))
        {
            fontdb.set_serif_family(name);
        }

        let bundled_font_name;
        #[cfg(feature = "roboto-flex")]
        {
            bundled_font_name = Some(String::from("Roboto Flex"));
        }
        #[cfg(not(feature = "roboto-flex"))]
        {
            bundled_font_name = None;
        }

        if let Some(FamilyOwned::Name(name)) = fonts
            .find_available_font_family(&settings.sans_serif_font_family)
            .or_else(|| {
                if let Some(name) = bundled_font_name {
                    Some(FamilyOwned::Name(name))
                } else {
                    default_family(Family::SansSerif)
                }
            })
        {
            fontdb.set_sans_serif_family(name);
        }

        if let Some(FamilyOwned::Name(name)) = fonts
            .find_available_font_family(&settings.fantasy_font_family)
            .or_else(|| default_family(Family::Fantasy))
        {
            fontdb.set_fantasy_family(name);
        }
        if let Some(FamilyOwned::Name(name)) = fonts
            .find_available_font_family(&settings.monospace_font_family)
            .or_else(|| default_family(Family::Monospace))
        {
            fontdb.set_monospace_family(name);
        }
        if let Some(FamilyOwned::Name(name)) = fonts
            .find_available_font_family(&settings.cursive_font_family)
            .or_else(|| default_family(Family::Cursive))
        {
            fontdb.set_cursive_family(name);
        }

        let theme_mode = match settings.theme_mode.take() {
            Some(Value::Dynamic(dynamic)) => {
                dynamic.set(window.theme().into());
                Value::Dynamic(dynamic)
            }
            Some(Value::Constant(mode)) => Value::Constant(mode),
            None => Value::dynamic(window.theme().into()),
        };
        let transparent = settings.transparent;
        let mut behavior = T::initialize(
            &mut RunningWindow::new(window, &gooey, &focused, &occluded),
            context.user,
        );
        let root = Tree::default().push_boxed(behavior.make_root(), None);

        let (current_theme, theme) = match theme {
            Value::Constant(theme) => (theme, None),
            Value::Dynamic(dynamic) => (dynamic.get(), Some(dynamic.into_reader())),
        };

        Self {
            behavior,
            root,
            contents: Drawing::default(),
            should_close: false,
            cursor: CursorState {
                location: None,
                widget: None,
            },
            mouse_buttons: AHashMap::default(),
            redraw_status: InvalidationStatus::default(),
            initial_frame: true,
            occluded,
            focused,
            keyboard_activated: None,
            min_inner_size: None,
            max_inner_size: None,
            current_theme,
            theme,
            theme_mode,
            transparent,
            fonts,
            gooey,
        }
    }

    fn prepare(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        graphics: &mut kludgine::Graphics<'_>,
    ) {
        if let Some(theme) = &mut self.theme {
            if theme.has_updated() {
                self.current_theme = theme.get();
                self.root.invalidate();
            }
        }

        self.redraw_status.refresh_received();
        graphics.reset_text_attributes();
        self.root
            .tree
            .new_frame(self.redraw_status.invalidations().drain());

        let resizable = window.winit().is_resizable();
        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
        let root_mode = self.constrain_window_resizing(resizable, &mut window, graphics);

        self.fonts.next_frame();
        let graphics = self.contents.new_frame(graphics);
        let mut context = GraphicsContext {
            widget: WidgetContext::new(
                self.root.clone(),
                &self.redraw_status,
                &self.current_theme,
                &mut window,
                self.theme_mode.get(),
                &mut self.cursor,
            ),
            gfx: Exclusive::Owned(Graphics::new(graphics, &mut self.fonts)),
        };
        if self.initial_frame {
            self.root
                .lock()
                .as_widget()
                .mounted(&mut context.as_event_context());
        }
        self.theme_mode.redraw_when_changed(&context);
        let mut layout_context = LayoutContext::new(&mut context);
        let window_size = layout_context.gfx.size();

        if !self.transparent {
            let background_color = layout_context.theme().surface.color;
            layout_context.graphics.gfx.fill(background_color);
        }

        let layout_size =
            layout_context.layout(if matches!(root_mode, RootMode::Expand | RootMode::Align) {
                window_size.map(ConstraintLimit::Fill)
            } else {
                window_size.map(ConstraintLimit::SizeToFit)
            });
        let actual_size = if root_mode == RootMode::Align {
            window_size.max(layout_size)
        } else {
            layout_size
        };
        let render_size = actual_size.min(window_size);
        if actual_size != window_size && !resizable {
            let mut new_size = actual_size;
            if let Some(min_size) = self.min_inner_size {
                new_size = new_size.max(min_size);
            }
            if let Some(max_size) = self.max_inner_size {
                new_size = new_size.min(max_size);
            }
            let _ = layout_context
                .winit()
                .request_inner_size(PhysicalSize::from(new_size));
        } else if render_size != actual_size && resizable {
            let _ = layout_context
                .winit()
                .request_inner_size(PhysicalSize::from(actual_size));
        }
        self.root.set_layout(Rect::from(render_size.into_signed()));

        if self.initial_frame {
            self.initial_frame = false;
            self.root
                .lock()
                .as_widget()
                .mounted(&mut layout_context.as_event_context());
            layout_context.focus();
            layout_context.as_event_context().apply_pending_state();
        }

        if render_size.width < window_size.width || render_size.height < window_size.height {
            layout_context
                .clipped_to(Rect::from(render_size.into_signed()))
                .redraw();
        } else {
            layout_context.redraw();
        }
    }

    fn focus_changed(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
    ) {
        self.focused.set(window.focused());
    }

    fn occlusion_changed(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
    ) {
        self.occluded.set(window.ocluded());
    }

    fn render<'pass>(
        &'pass mut self,
        _window: kludgine::app::Window<'_, WindowCommand>,
        graphics: &mut kludgine::RenderingGraphics<'_, 'pass>,
    ) -> bool {
        self.contents.render(1., graphics);

        !self.should_close
    }

    fn initial_window_attributes(context: &Self::Context) -> kludgine::app::WindowAttributes {
        let mut attrs = context
            .settings
            .borrow_mut()
            .attributes
            .take()
            .expect("called more than once");
        if let Some(Value::Constant(theme_mode)) = &context.settings.borrow().theme_mode {
            attrs.preferred_theme = Some((*theme_mode).into());
        }
        attrs
    }

    fn close_requested(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
    ) -> bool {
        Self::request_close(
            &mut self.should_close,
            &mut self.behavior,
            &mut RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded),
        )
    }

    // fn power_preference() -> wgpu::PowerPreference {
    //     wgpu::PowerPreference::default()
    // }

    // fn limits(adapter_limits: wgpu::Limits) -> wgpu::Limits {
    //     wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter_limits)
    // }

    fn clear_color(&self) -> Option<kludgine::Color> {
        Some(if self.transparent {
            kludgine::Color::CLEAR_BLACK
        } else {
            kludgine::Color::BLACK
        })
    }

    fn composite_alpha_mode(&self, supported_modes: &[CompositeAlphaMode]) -> CompositeAlphaMode {
        if self.transparent && supported_modes.contains(&CompositeAlphaMode::PreMultiplied) {
            CompositeAlphaMode::PreMultiplied
        } else {
            CompositeAlphaMode::Auto
        }
    }

    // fn focus_changed(&mut self, window: kludgine::app::Window<'_, ()>) {}

    // fn occlusion_changed(&mut self, window: kludgine::app::Window<'_, ()>) {}

    // fn scale_factor_changed(&mut self, window: kludgine::app::Window<'_, ()>) {}

    fn resized(
        &mut self,
        _window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
    ) {
        self.root.invalidate();
    }

    // fn theme_changed(&mut self, window: kludgine::app::Window<'_, ()>) {}

    // fn dropped_file(&mut self, window: kludgine::app::Window<'_, ()>, path: std::path::PathBuf) {}

    // fn hovered_file(&mut self, window: kludgine::app::Window<'_, ()>, path: std::path::PathBuf) {}

    // fn hovered_file_cancelled(&mut self, window: kludgine::app::Window<'_, ()>) {}

    // fn received_character(&mut self, window: kludgine::app::Window<'_, ()>, char: char) {}

    fn keyboard_input(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        device_id: DeviceId,
        input: KeyEvent,
        is_synthetic: bool,
    ) {
        let target = self.root.tree.focused_widget().unwrap_or(self.root.node_id);
        let Some(target) = self.root.tree.widget_from_node(target) else {
            return;
        };
        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
        let mut target = EventContext::new(
            WidgetContext::new(
                target,
                &self.redraw_status,
                &self.current_theme,
                &mut window,
                self.theme_mode.get(),
                &mut self.cursor,
            ),
            kludgine,
        );

        let handled = recursively_handle_event(&mut target, |widget| {
            widget.keyboard_input(device_id, input.clone(), is_synthetic)
        })
        .is_some();
        drop(target);

        if !handled {
            match input.logical_key {
                Key::Character(ch) if ch == "w" && window.modifiers().primary() => {
                    if input.state.is_pressed()
                        && Self::request_close(
                            &mut self.should_close,
                            &mut self.behavior,
                            &mut window,
                        )
                    {
                        window.set_needs_redraw();
                    }
                }
                Key::Named(NamedKey::Tab) if !window.modifiers().possible_shortcut() => {
                    if input.state.is_pressed() {
                        let reverse = window.modifiers().state().shift_key();

                        let target = self.root.tree.focused_widget().unwrap_or(self.root.node_id);
                        let target = self
                            .root
                            .tree
                            .widget_from_node(target)
                            .expect("missing widget");
                        let mut target = EventContext::new(
                            WidgetContext::new(
                                target,
                                &self.redraw_status,
                                &self.current_theme,
                                &mut window,
                                self.theme_mode.get(),
                                &mut self.cursor,
                            ),
                            kludgine,
                        );

                        if reverse {
                            target.return_focus();
                        } else {
                            target.advance_focus();
                        }
                    }
                }
                Key::Named(NamedKey::Enter) => {
                    self.keyboard_activate_widget(
                        input.state.is_pressed(),
                        self.root.tree.default_widget(),
                        &mut window,
                        kludgine,
                    );
                }
                Key::Named(NamedKey::Escape) => {
                    self.keyboard_activate_widget(
                        input.state.is_pressed(),
                        self.root.tree.escape_widget(),
                        &mut window,
                        kludgine,
                    );
                }
                _ => {
                    tracing::event!(
                        Level::DEBUG,
                        logical = ?input.logical_key,
                        physical = ?input.physical_key,
                        state = ?input.state,
                        "Ignored Keyboard Input",
                    );
                }
            }
        }
    }

    fn mouse_wheel(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    ) {
        let widget = self
            .root
            .tree
            .hovered_widget()
            .and_then(|hovered| self.root.tree.widget_from_node(hovered))
            .unwrap_or_else(|| {
                self.root
                    .tree
                    .widget(self.root.id())
                    .expect("missing widget")
            });

        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
        let mut widget = EventContext::new(
            WidgetContext::new(
                widget,
                &self.redraw_status,
                &self.current_theme,
                &mut window,
                self.theme_mode.get(),
                &mut self.cursor,
            ),
            kludgine,
        );
        recursively_handle_event(&mut widget, |widget| {
            widget.mouse_wheel(device_id, delta, phase)
        });
    }

    // fn modifiers_changed(&mut self, window: kludgine::app::Window<'_, ()>) {}

    fn ime(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        ime: Ime,
    ) {
        let widget = self
            .root
            .tree
            .focused_widget()
            .and_then(|hovered| self.root.tree.widget_from_node(hovered))
            .unwrap_or_else(|| {
                self.root
                    .tree
                    .widget(self.root.id())
                    .expect("missing widget")
            });
        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
        let mut target = EventContext::new(
            WidgetContext::new(
                widget,
                &self.redraw_status,
                &self.current_theme,
                &mut window,
                self.theme_mode.get(),
                &mut self.cursor,
            ),
            kludgine,
        );

        let _handled =
            recursively_handle_event(&mut target, |widget| widget.ime(ime.clone())).is_some();
    }

    fn cursor_moved(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        device_id: DeviceId,
        position: PhysicalPosition<f64>,
    ) {
        let location = Point::<Px>::from(position);
        self.cursor.location = Some(location);

        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);

        EventContext::new(
            WidgetContext::new(
                self.root.clone(),
                &self.redraw_status,
                &self.current_theme,
                &mut window,
                self.theme_mode.get(),
                &mut self.cursor,
            ),
            kludgine,
        )
        .update_hovered_widget();

        if let Some(state) = self.mouse_buttons.get(&device_id) {
            // Mouse Drag
            for (button, handler) in state {
                let Some(handler) = self.root.tree.widget(*handler) else {
                    continue;
                };
                let mut context = EventContext::new(
                    WidgetContext::new(
                        handler.clone(),
                        &self.redraw_status,
                        &self.current_theme,
                        &mut window,
                        self.theme_mode.get(),
                        &mut self.cursor,
                    ),
                    kludgine,
                );
                let Some(last_rendered_at) = context.last_layout() else {
                    continue;
                };
                context.mouse_drag(location - last_rendered_at.origin, device_id, *button);
            }
        }
    }

    fn cursor_left(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        _device_id: DeviceId,
    ) {
        if self.cursor.widget.take().is_some() {
            let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
            let mut context = EventContext::new(
                WidgetContext::new(
                    self.root.clone(),
                    &self.redraw_status,
                    &self.current_theme,
                    &mut window,
                    self.theme_mode.get(),
                    &mut self.cursor,
                ),
                kludgine,
            );
            context.clear_hover();
        }
    }

    fn mouse_input(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        kludgine: &mut Kludgine,
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    ) {
        let mut window = RunningWindow::new(window, &self.gooey, &self.focused, &self.occluded);
        match state {
            ElementState::Pressed => {
                EventContext::new(
                    WidgetContext::new(
                        self.root.clone(),
                        &self.redraw_status,
                        &self.current_theme,
                        &mut window,
                        self.theme_mode.get(),
                        &mut self.cursor,
                    ),
                    kludgine,
                )
                .clear_focus();

                if let (ElementState::Pressed, Some(location), Some(hovered)) = (
                    state,
                    self.cursor.location,
                    self.cursor.widget.and_then(|id| self.root.tree.widget(id)),
                ) {
                    if let Some(handler) = recursively_handle_event(
                        &mut EventContext::new(
                            WidgetContext::new(
                                hovered.clone(),
                                &self.redraw_status,
                                &self.current_theme,
                                &mut window,
                                self.theme_mode.get(),
                                &mut self.cursor,
                            ),
                            kludgine,
                        ),
                        |context| {
                            let Some(layout) = context.last_layout() else {
                                return IGNORED;
                            };
                            let relative = location - layout.origin;
                            context.mouse_down(relative, device_id, button)
                        },
                    ) {
                        self.mouse_buttons
                            .entry(device_id)
                            .or_default()
                            .insert(button, handler.id());
                    }
                }
            }
            ElementState::Released => {
                let Some(device_buttons) = self.mouse_buttons.get_mut(&device_id) else {
                    return;
                };
                let Some(handler) = device_buttons.remove(&button) else {
                    return;
                };
                if device_buttons.is_empty() {
                    self.mouse_buttons.remove(&device_id);
                }
                let Some(handler) = self.root.tree.widget(handler) else {
                    return;
                };
                let cursor_location = self.cursor.location;
                let mut context = EventContext::new(
                    WidgetContext::new(
                        handler,
                        &self.redraw_status,
                        &self.current_theme,
                        &mut window,
                        self.theme_mode.get(),
                        &mut self.cursor,
                    ),
                    kludgine,
                );

                let relative = if let (Some(last_rendered), Some(location)) =
                    (context.last_layout(), cursor_location)
                {
                    Some(location - last_rendered.origin)
                } else {
                    None
                };

                context.mouse_up(relative, device_id, button);
            }
        }
    }

    fn theme_changed(
        &mut self,
        window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
    ) {
        if let Value::Dynamic(theme_mode) = &self.theme_mode {
            theme_mode.set(window.theme().into());
        }
    }

    fn event(
        &mut self,
        mut window: kludgine::app::Window<'_, WindowCommand>,
        _kludgine: &mut Kludgine,
        event: WindowCommand,
    ) {
        match event {
            WindowCommand::Redraw => {
                window.set_needs_redraw();
            }
        }
    }
}

fn recursively_handle_event(
    context: &mut EventContext<'_, '_>,
    mut each_widget: impl FnMut(&mut EventContext<'_, '_>) -> EventHandling,
) -> Option<MountedWidget> {
    match each_widget(context) {
        HANDLED => Some(context.widget().clone()),
        IGNORED => context.parent().and_then(|parent| {
            recursively_handle_event(&mut context.for_other(&parent), each_widget)
        }),
    }
}

#[derive(Default)]
pub(crate) struct CursorState {
    pub(crate) location: Option<Point<Px>>,
    pub(crate) widget: Option<WidgetId>,
}

pub(crate) mod sealed {
    use std::cell::RefCell;

    use crate::styles::{FontFamilyList, ThemePair};
    use crate::value::{Dynamic, Value};
    use crate::window::{ThemeMode, WindowAttributes};
    use crate::Gooey;

    pub struct Context<C> {
        pub user: C,
        pub settings: RefCell<WindowSettings>,
    }

    pub struct WindowSettings {
        pub gooey: Gooey,
        pub attributes: Option<WindowAttributes>,
        pub occluded: Option<Dynamic<bool>>,
        pub focused: Option<Dynamic<bool>>,
        pub theme: Option<Value<ThemePair>>,
        pub theme_mode: Option<Value<ThemeMode>>,
        pub transparent: bool,
        pub serif_font_family: FontFamilyList,
        pub sans_serif_font_family: FontFamilyList,
        pub fantasy_font_family: FontFamilyList,
        pub monospace_font_family: FontFamilyList,
        pub cursive_font_family: FontFamilyList,
        pub font_data_to_load: Vec<Vec<u8>>,
    }

    #[derive(Clone)]
    pub enum WindowCommand {
        Redraw,
        // RequestClose,
    }
}

/// Controls whether the light or dark theme is applied.
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, LinearInterpolate)]
pub enum ThemeMode {
    /// Applies the light theme
    Light,
    /// Applies the dark theme
    #[default]
    Dark,
}

impl ThemeMode {
    /// Returns the opposite mode of `self`.
    #[must_use]
    pub const fn inverse(self) -> Self {
        match self {
            ThemeMode::Light => Self::Dark,
            ThemeMode::Dark => Self::Light,
        }
    }

    /// Updates `self` with its [inverse](Self::inverse).
    pub fn toggle(&mut self) {
        *self = !*self;
    }
}

impl Not for ThemeMode {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.inverse()
    }
}

impl From<window::Theme> for ThemeMode {
    fn from(value: window::Theme) -> Self {
        match value {
            window::Theme::Light => Self::Light,
            window::Theme::Dark => Self::Dark,
        }
    }
}

impl From<ThemeMode> for window::Theme {
    fn from(value: ThemeMode) -> Self {
        match value {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
        }
    }
}

impl PercentBetween for ThemeMode {
    fn percent_between(&self, min: &Self, max: &Self) -> ZeroToOne {
        if *min == *max || *self == *min {
            ZeroToOne::ZERO
        } else {
            ZeroToOne::ONE
        }
    }
}

impl Ranged for ThemeMode {
    const MAX: Self = Self::Dark;
    const MIN: Self = Self::Light;
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
fn default_family(_query: Family<'_>) -> Option<FamilyOwned> {
    // fontdb uses system APIs to determine these defaults.
    None
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows")))]
fn default_family(query: Family<'_>) -> Option<FamilyOwned> {
    // fontdb does not yet support configuring itself automatically. We will try
    // to use `fc-match` to query font config. Once this is supported, we can
    // remove this functionality.
    // <https://github.com/RazrFalcon/fontdb/issues/24>
    let query = match query {
        Family::Serif => "serif",
        Family::SansSerif => "sans",
        Family::Cursive => "cursive",
        Family::Fantasy => "fantasy",
        Family::Monospace => "monospace",
        Family::Name(_) => return None,
    };

    std::process::Command::new("fc-match")
        .arg("-f")
        .arg("%{family}")
        .arg(query)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(FamilyOwned::Name)
}

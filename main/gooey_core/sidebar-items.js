initSidebarItems({"constant":[["ROOT_CLASS","The name of the class assigned to the root widget of a window."],["SOLID_WIDGET_CLASS","The name of the class assigned to widgets that have a solid background"]],"mod":[["assets",""],["styles","Types used for styling."]],"struct":[["AnyTransmogrifierContext","A context used internally when the [`Transmogrifier`] type cannot be known."],["Callback","A callback that receives information `I`, and returns `R`."],["Channels","Communication channels used to communicate between [`Widget`]s and `Transmogrifier`s."],["Context","Enables [`Widget`]s to send commands to the `Transmogrifier`."],["Gooey","A graphical user interface."],["ManagedCodeGuard","A guard marking that Gooey-managed code is executing."],["Pixels","A unit representing physical pixels on a display."],["Points","A unit aiming to represent the scaled resolution of the display the interface is being displayed on. The ratio between [`Pixels`] and `Points` can vary based on many things, including the display configuration, the system user interface settings, and the browser’s zoom level. Each [`Frontend`] will use its best available methods for translating `Points` to [`Pixels`] in a way that is consistent with other applications."],["StyledWidget","A widget and its initial style information."],["TransmogrifierContext","A context passed into [`Transmogrifier`] functions with access to useful data and types. This type is mostly used to avoid passing so many parameters across all functions."],["TransmogrifierState","Generic storage for a transmogrifier."],["Transmogrifiers","A collection of transmogrifiers to use inside of a frontend."],["WeakWidgetRegistration","References an initialized widget. These references will not keep a widget from being removed."],["WidgetId","A unique ID of a widget, with information about the widget type."],["WidgetRef","A widget reference. Does not prevent a widget from being destroyed if removed from an interface."],["WidgetRegistration","References an initialized widget. On drop, frees the storage and id."],["WidgetState","Generic, clone-able storage for a widget’s transmogrifier."],["WidgetStorage","Generic-type-less widget storage."]],"trait":[["AnyChannels","A generic-type-less trait for [`Channels`]"],["AnyFrontend","An interface for Frontend that doesn’t requier knowledge of associated types."],["AnySendSync","A value that can be used as [`Any`] that is threadsafe."],["AnyTransmogrifier","A Transmogrifier without any associated types."],["AnyWidget","A Widget without any associated types. Useful for implementing frontends."],["CallbackFn","A callback implementation. Not typically directly implemented, as this trait is auto-implemented for any `Fn(I) -> R` types."],["DefaultWidget","A widget that can be created with defaults."],["Frontend","A frontend is an implementation of widgets and layouts."],["Key","A key for a widget."],["KeyedStorage","A type that registers widgets with an associated key."],["RelatedStorage","Related storage enables a widget to communicate in a limited way about widgets being inserted or removed."],["Transmogrifier","Transforms a Widget into whatever is needed for [`Frontend`] `F`."],["Widget","A graphical user interface element."]]});
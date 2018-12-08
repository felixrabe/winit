use std::{
    self, f64, os::raw::c_void,
    sync::{Arc, atomic::{Ordering, AtomicBool}, Mutex, Weak},
};

use cocoa::{
    appkit::{
        self, CGFloat, NSApp, NSApplication, NSApplicationActivationPolicy,
        NSColor, NSRequestUserAttentionType, NSScreen, NSView,
        NSWindow, NSWindowButton, NSWindowStyleMask,
    },
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSDictionary, NSPoint, NSRect, NSSize, NSString},
};
use core_graphics::display::CGDisplay;
use objc::{runtime::{Class, Object, Sel, BOOL, YES, NO}, declare::ClassDecl};

use {
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    window::{CreationError, MouseCursor, WindowAttributes},
};
use platform::macos::{ActivationPolicy, WindowExtMacOS};
use platform_impl::platform::{
    {ffi, util::{self, Access, IdRef}},
    event_loop::{EventLoop, EventLoopAccess},
    monitor::MonitorHandle,
    view::{new_view, set_ime_spot},
    window_delegate::WindowDelegate,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

// Convert the `cocoa::base::id` associated with a window to a usize to use as a unique identifier
// for the window.
pub fn get_window_id(window_cocoa_id: id) -> Id {
    Id(window_cocoa_id as *const Object as _)
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub activation_policy: ActivationPolicy,
    pub movable_by_window_background: bool,
    pub titlebar_transparent: bool,
    pub title_hidden: bool,
    pub titlebar_hidden: bool,
    pub titlebar_buttons_hidden: bool,
    pub fullsize_content_view: bool,
    pub resize_increments: Option<LogicalSize>,
}

fn create_app(activation_policy: ActivationPolicy) -> Option<id> {
    unsafe {
        let app = NSApp();
        if app == nil {
            None
        } else {
            use self::NSApplicationActivationPolicy::*;
            let ns_activation_policy = match activation_policy {
                ActivationPolicy::Regular => NSApplicationActivationPolicyRegular,
                ActivationPolicy::Accessory => NSApplicationActivationPolicyAccessory,
                ActivationPolicy::Prohibited => NSApplicationActivationPolicyProhibited,
            };
            app.setActivationPolicy_(ns_activation_policy);
            app.finishLaunching();
            Some(app)
        }
    }
}

unsafe fn create_view(window: id, pending_events: Weak<PendingEvents>) -> Option<IdRef> {
    new_view(window, pending_events).non_nil().map(|view| {
        view.setWantsBestResolutionOpenGLSurface_(YES);

        // On Mojave, views automatically become layer-backed shortly after being added to
        // a window. Changing the layer-backedness of a view breaks the association between
        // the view and its associated OpenGL context. To work around this, on Mojave we
        // explicitly make the view layer-backed up front so that AppKit doesn't do it
        // itself and break the association with its context.
        if f64::floor(appkit::NSAppKitVersionNumber) > appkit::NSAppKitVersionNumber10_12 {
            view.setWantsLayer(YES);
        }

        window.setContentView_(*view);
        window.makeFirstResponder_(*view);
        view
    })
}

#[derive(Default)]
pub struct SharedState {
    pub resizable: bool,
    pub fullscreen: Option<MonitorHandle>,
    pub maximized: bool,
    standard_frame: Option<NSRect>,
    saved_style: Option<NSWindowStyleMask>,
}

impl From<WindowAttributes> for SharedState {
    fn from(attribs: WindowAttributes) -> Self {
        SharedState {
            resizable: attribs.resizable,
            fullscreen: attribs.fullscreen,
            maximized: attribs.maximized,
            .. Default::default()
        }
    }
}

pub struct UnownedWindow {
    nswindow: IdRef, // never changes
    nsview: IdRef, // never changes
    input_context: IdRef, // never changes
    pub shared_state: Mutex<SharedState>,
    cursor_hidden: AtomicBool,
}

unsafe impl Send for UnownedWindow {}
unsafe impl Sync for UnownedWindow {}

impl UnownedWindow {
    pub fn new<T: 'static>(
        ev_access: Weak<Mutex<EventLoopAccess>>,
        win_attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, CreationError> {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("Windows can only be created on the main thread on macOS");
            }
        }

        let autoreleasepool = unsafe { NSAutoreleasePool::new(nil) };

        let nsapp = create_app(pl_attribs.activation_policy).ok_or_else(|| {
            let _: () = unsafe { msg_send![autoreleasepool, drain] };
            CreationError::OsError(format!("Couldn't create `NSApplication`"))
        })?;

        let nswindow = Self::create_window(&win_attribs, &pl_attribs).ok_or_else(|| {
            let _: () = unsafe { msg_send![autoreleasepool, drain] };
            CreationError::OsError(format!("Couldn't create `NSWindow`"))
        })?;

        let nsview = unsafe { create_view(*nswindow, Weak::clone(&ev_access)) }.ok_or_else(|| {
            let _: () = unsafe { msg_send![autoreleasepool, drain] };
            CreationError::OsError(format!("Couldn't create `NSView`"))
        })?;

        let input_context = unsafe { util::create_input_context(*nsview) };

        unsafe {
            if win_attribs.transparent {
                nswindow.setOpaque_(NO);
                nswindow.setBackgroundColor_(NSColor::clearColor(nil));
            }

            nsapp.activateIgnoringOtherApps_(YES);

            win_attribs.min_dimensions.map(|dim| set_min_dimensions(*window, dim));
            win_attribs.max_dimensions.map(|dim| set_max_dimensions(*window, dim));

            use cocoa::foundation::NSArray;
            // register for drag and drop operations.
            let () = msg_send![(*window as id),
                registerForDraggedTypes:NSArray::arrayWithObject(nil, appkit::NSFilenamesPboardType)];
        }

        let fullscreen = win_attribs.fullscreen;
        let maximized = win_attribs.maximized;
        let visible = win_attribs.visible;

        let window = UnownedWindow {
            view,
            window,
            input_context,
            shared_state: Mutex::new(win_attribs.into()),
            cursor_hidden: Default::default(),
        };

        let delegate = {
            let dpi_factor = window.get_hidpi_factor();

            let mut delegate_state = WindowDelegateState {
                window: Arc::downgrade(&window),
                ev_access: Weak::clone(&ev_access),
                handle_with_fullscreen: fullscreen.is_some(),
                previous_position: None,
                previous_dpi_factor: dpi_factor,
            };
            // What's this?
            delegate_state.win_attribs.borrow_mut().fullscreen = None;

            if dpi_factor != 1.0 {
                delegate_state.emit_event(WindowEvent::HiDpiFactorChanged(dpi_factor));
                delegate_state.emit_resize_event();
            }

            WindowDelegate::new(delegate_state)
        };

        // Set fullscreen mode after we setup everything
        if let Some(ref monitor) = fullscreen {
            unsafe {
                if monitor.inner != get_current_monitor(*window.nswindow).inner {
                    // To do this with native fullscreen, we probably need to warp the window...
                    unimplemented!();
                }
            }
            window.set_fullscreen(Some(monitor.clone()));
        }

        // Make key have to be after set fullscreen
        // to prevent normal size window brefly appears
        unsafe {
            if visible {
                window.nswindow.makeKeyAndOrderFront_(nil);
            } else {
                window.nswindow.makeKeyWindow();
            }
        }

        if maximized {
            delegate.state.perform_maximized(maximized);
        }

        let _: () = unsafe { msg_send![autoreleasepool, drain] };

        Ok(window)
    }

    pub fn id(&self) -> Id {
        get_window_id(*self.nswindow)
    }

    fn class() -> *const Class {
        static mut WINDOW2_CLASS: *const Class = 0 as *const Class;
        static INIT: std::sync::Once = std::sync::ONCE_INIT;

        INIT.call_once(|| unsafe {
            let window_superclass = class!(NSWindow);
            let mut decl = ClassDecl::new("WinitWindow", window_superclass).unwrap();
            decl.add_method(sel!(canBecomeMainWindow), util::yes as extern fn(&Object, Sel) -> BOOL);
            decl.add_method(sel!(canBecomeKeyWindow), util::yes as extern fn(&Object, Sel) -> BOOL);
            WINDOW2_CLASS = decl.register();
        });

        unsafe {
            WINDOW2_CLASS
        }
    }

    fn create_window(
        attrs: &WindowAttributes,
        pl_attrs: &PlatformSpecificWindowBuilderAttributes
    ) -> Option<IdRef> {
        unsafe {
            let autoreleasepool = NSAutoreleasePool::new(nil);
            let screen = match attrs.fullscreen {
                Some(ref monitor_id) => {
                    let monitor_screen = monitor_id.inner.get_nsscreen();
                    Some(monitor_screen.unwrap_or(appkit::NSScreen::mainScreen(nil)))
                },
                _ => None,
            };
            let frame = match screen {
                Some(screen) => appkit::NSScreen::frame(screen),
                None => {
                    let (width, height) = attrs.dimensions
                        .map(|logical| (logical.width, logical.height))
                        .unwrap_or((800.0, 600.0));
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
                }
            };

            let mut masks = if !attrs.decorations && !screen.is_some() {
                // Resizable UnownedWindow without a titlebar or borders
                // if decorations is set to false, ignore pl_attrs
                NSWindowStyleMask::NSBorderlessWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
            } else if pl_attrs.titlebar_hidden {
                // if the titlebar is hidden, ignore other pl_attrs
                NSWindowStyleMask::NSBorderlessWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask
            } else {
                // default case, resizable window with titlebar and titlebar buttons
                NSWindowStyleMask::NSClosableWindowMask |
                    NSWindowStyleMask::NSMiniaturizableWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask
            };

            if !attrs.resizable {
                masks &= !NSWindowStyleMask::NSResizableWindowMask;
            }

            if pl_attrs.fullsize_content_view {
                masks |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
            }

            let winit_window = UnownedWindow::class();

            let window: id = msg_send![winit_window, alloc];

            let window = IdRef::new(window.initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                appkit::NSBackingStoreBuffered,
                NO,
            ));
            let res = window.non_nil().map(|window| {
                let title = IdRef::new(NSString::alloc(nil).init_str(&attrs.title));
                window.setReleasedWhenClosed_(NO);
                window.setTitle_(*title);
                window.setAcceptsMouseMovedEvents_(YES);

                if pl_attrs.titlebar_transparent {
                    window.setTitlebarAppearsTransparent_(YES);
                }
                if pl_attrs.title_hidden {
                    window.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
                }
                if pl_attrs.titlebar_buttons_hidden {
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowFullScreenButton);
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowMiniaturizeButton);
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowCloseButton);
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowZoomButton);
                    let () = msg_send![button, setHidden:YES];
                }
                if pl_attrs.movable_by_window_background {
                    window.setMovableByWindowBackground_(YES);
                }

                if attrs.always_on_top {
                    let _: () = msg_send![*window, setLevel:ffi::NSWindowLevel::NSFloatingWindowLevel];
                }

                if let Some(increments) = pl_attrs.resize_increments {
                    let (x, y) = (increments.width, increments.height);
                    if x >= 1.0 && y >= 1.0 {
                        let size = NSSize::new(x as CGFloat, y as CGFloat);
                        window.setResizeIncrements_(size);
                    }
                }

                window.center();
                window
            });
            let _: () = msg_send![autoreleasepool, drain];
            res
        }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = IdRef::new(NSString::alloc(nil).init_str(title));
            self.nswindow.setTitle_(*title);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe { NSWindow::makeKeyAndOrderFront_(*self.nswindow, nil); }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe { NSWindow::orderOut_(*self.nswindow, nil); }
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let frame_rect = unsafe { NSWindow::frame(*self.nswindow) };
        Some((
            frame_rect.origin.x as f64,
            util::bottom_left_to_top_left(frame_rect),
        ).into())
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        let content_rect = unsafe {
            NSWindow::contentRectForFrameRect_(
                *self.nswindow,
                NSWindow::frame(*self.nswindow),
            )
        };
        Some((
            content_rect.origin.x as f64,
            util::bottom_left_to_top_left(content_rect),
        ).into())
    }

    pub fn set_position(&self, position: LogicalPosition) {
        let dummy = NSRect::new(
            NSPoint::new(
                position.x,
                // While it's true that we're setting the top-left position, it still needs to be
                // in a bottom-left coordinate system.
                CGDisplay::main().pixels_high() as f64 - position.y,
            ),
            NSSize::new(0f64, 0f64),
        );
        unsafe {
            NSWindow::setFrameTopLeftPoint_(*self.nswindow, dummy.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSView::frame(*self.nsview) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSWindow::frame(*self.nswindow) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unsafe {
            NSWindow::setContentSize_(*self.nswindow, NSSize::new(size.width as CGFloat, size.height as CGFloat));
        }
    }

    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (0, 0).into());
            set_min_dimensions(*self.nswindow, dimensions);
        }
    }

    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (!0, !0).into());
            set_max_dimensions(*self.nswindow, dimensions);
        }
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let mut shared_state_lock = self.shared_state.lock().unwrap();
        shared_state_lock.resizable = resizable;
        if shared_state_lock.fullscreen.is_none() {
            let mut mask = unsafe { self.nswindow.styleMask() };
            if resizable {
                mask |= NSWindowStyleMask::NSResizableWindowMask;
            } else {
                mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            unsafe { util::set_style_mask(*self.nswindow, *self.nsview, mask) };
        } // Otherwise, we don't change the mask until we exit fullscreen.
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_name = match cursor {
            MouseCursor::Arrow | MouseCursor::Default => "arrowCursor",
            MouseCursor::Hand => "pointingHandCursor",
            MouseCursor::Grabbing | MouseCursor::Grab => "closedHandCursor",
            MouseCursor::Text => "IBeamCursor",
            MouseCursor::VerticalText => "IBeamCursorForVerticalLayout",
            MouseCursor::Copy => "dragCopyCursor",
            MouseCursor::Alias => "dragLinkCursor",
            MouseCursor::NotAllowed | MouseCursor::NoDrop => "operationNotAllowedCursor",
            MouseCursor::ContextMenu => "contextualMenuCursor",
            MouseCursor::Crosshair => "crosshairCursor",
            MouseCursor::EResize => "resizeRightCursor",
            MouseCursor::NResize => "resizeUpCursor",
            MouseCursor::WResize => "resizeLeftCursor",
            MouseCursor::SResize => "resizeDownCursor",
            MouseCursor::EwResize | MouseCursor::ColResize => "resizeLeftRightCursor",
            MouseCursor::NsResize | MouseCursor::RowResize => "resizeUpDownCursor",

            // TODO: Find appropriate OSX cursors
            MouseCursor::NeResize | MouseCursor::NwResize |
            MouseCursor::SeResize | MouseCursor::SwResize |
            MouseCursor::NwseResize | MouseCursor::NeswResize |

            MouseCursor::Cell |
            MouseCursor::Wait | MouseCursor::Progress | MouseCursor::Help |
            MouseCursor::Move | MouseCursor::AllScroll | MouseCursor::ZoomIn |
            MouseCursor::ZoomOut => "arrowCursor",
        };
        let sel = Sel::register(cursor_name);
        let cls = class!(NSCursor);
        unsafe {
            use objc::Message;
            let cursor: id = cls.send_message(sel, ()).unwrap();
            let _: () = msg_send![cursor, set];
        }
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        // TODO: Do this for real https://stackoverflow.com/a/40922095/5435443
        CGDisplay::associate_mouse_and_mouse_cursor_position(!grab)
            .map_err(|status| format!("Failed to grab cursor: `CGError` {:?}", status))
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        let cursor_class = class!(NSCursor);
        // macOS uses a "hide counter" like Windows does, so we avoid incrementing it more than once.
        // (otherwise, `hide_cursor(false)` would need to be called n times!)
        if hide != self.cursor_hidden.load(Ordering::Acquire) {
            if hide {
                let _: () = unsafe { msg_send![cursor_class, hide] };
            } else {
                let _: () = unsafe { msg_send![cursor_class, unhide] };
            }
            self.cursor_hidden.store(hide, Ordering::Release);
        }
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        unsafe {
            NSWindow::backingScaleFactor(*self.nswindow) as f64
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, cursor_position: LogicalPosition) -> Result<(), String> {
        let window_position = self.get_inner_position()
            .ok_or("`get_inner_position` failed".to_owned())?;
        let point = appkit::CGPoint {
            x: (cursor_position.x + window_position.x) as CGFloat,
            y: (cursor_position.y + window_position.y) as CGFloat,
        };
        CGDisplay::warp_mouse_cursor_position(point)
            .map_err(|e| format!("`CGWarpMouseCursorPosition` failed: {:?}", e))?;
        CGDisplay::associate_mouse_and_mouse_cursor_position(true)
            .map_err(|e| format!("`CGAssociateMouseAndMouseCursorPosition` failed: {:?}", e))?;

        Ok(())
    }

    pub(crate) fn is_zoomed(&self) -> bool {
        // because `isZoomed` doesn't work if the window's borderless,
        // we make it resizable temporalily.
        let curr_mask = self.nswindow.styleMask();

        let required = NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSResizableWindowMask;
        let needs_temp_mask = !curr_mask.contains(required);
        if needs_temp_mask {
            unsafe { util::set_style_mask(*self.nswindow, *self.nsview, required) };
        }

        let is_zoomed: BOOL = unsafe { msg_send![*self.nswindow, isZoomed] };

        // Roll back temp styles
        if needs_temp_mask {
            unsafe { util::set_style_mask(*self.nswindow, *self.nsview, curr_mask) };
        }

        is_zoomed != 0
    }

    pub(crate) fn restore_state_from_fullscreen(&self) {
        self.set_maximized({
            let mut shared_state_lock = self.shared_state.lock().unwrap();

            shared_state_lock.fullscreen = None;

            let mask = {
                let base_mask = shared_state_lock.saved_style
                    .take()
                    .unwrap_or_else(|| self.nswindow.styleMask());
                if shared_state_lock.resizable {
                    base_mask | NSWindowStyleMask::NSResizableWindowMask
                } else {
                    base_mask & !NSWindowStyleMask::NSResizableWindowMask
                }
            };

            unsafe { util::set_style_mask(*self.nswindow, *self.nsview, mask) };

            shared_state_lock.maximized
        });
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let is_zoomed = self.is_zoomed();
        if is_zoomed == maximized { return };

        let mut shared_state_lock = self.shared_state.lock().unwrap();

        // Save the standard frame sized if it is not zoomed
        if !is_zoomed {
            unsafe {
                shared_state_lock.standard_frame = Some(NSWindow::frame(*self.nswindow));
            }
        }

        shared_state_lock.maximized = maximized;

        let curr_mask = unsafe { self.nswindow.styleMask() };
        if shared_state_lock.fullscreen.is_some() {
            // Handle it in window_did_exit_fullscreen
            return;
        } else if curr_mask.contains(NSWindowStyleMask::NSResizableWindowMask) {
            // Just use the native zoom if resizable
            unsafe { self.nswindow.zoom_(nil) };
        } else {
            // if it's not resizable, we set the frame directly
            unsafe {
                let new_rect = if maximized {
                    let screen = NSScreen::mainScreen(nil);
                    NSScreen::visibleFrame(screen)
                } else {
                    shared_state_lock.standard_frame.unwrap_or_else(|| NSRect::new(
                        NSPoint::new(50.0, 50.0),
                        NSSize::new(800.0, 600.0),
                    ))
                };
                self.nswindow.setFrame_display_(new_rect, 0);
            }
        }
    }

    /// TODO: Right now set_fullscreen do not work on switching monitors
    /// in fullscreen mode
    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorHandle>) {
        let mut shared_state_lock = self.shared_state.lock().unwrap();

        let current = {
            let current = shared_state_lock.fullscreen.clone();
            match (&current, monitor) {
                (&None, None) => {
                    return;
                }
                (&Some(ref a), Some(ref b)) if a.inner != b.inner => {
                    unimplemented!();
                }
                (&Some(_), Some(_)) => {
                    return;
                }
                _ => (),
            }

            current
        };

        unsafe {
            // Because toggleFullScreen will not work if the StyleMask is none,
            // We set a normal style to it temporary.
            // It will clean up at window_did_exit_fullscreen.
            if current.is_none() {
                let curr_mask = self.nswindow.styleMask();
                let required = NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask;
                if !curr_mask.contains(required) {
                    util::set_style_mask(*self.nswindow, *self.nsview, required);
                    shared_state_lock.saved_style = Some(curr_mask);
                }
            }
            self.nswindow.toggleFullScreen_(nil);
        }
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let mut shared_state_lock = self.shared_state.lock().unwrap();

        if shared_state_lock.decorations == decorations { return };

        shared_state_lock.decorations = decorations;

        // Skip modifiy if we are in fullscreen mode,
        // window_did_exit_fullscreen will handle it
        if shared_state_lock.fullscreen.is_some() { return };

        unsafe {
            let mut new_mask = if decorations {
                NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
                    | NSWindowStyleMask::NSTitledWindowMask
            } else {
                NSWindowStyleMask::NSBorderlessWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
            };
            if !shared_state_lock.resizable {
                new_mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            util::set_style_mask(*self.nswindow, *self.nsview, new_mask);
        }
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        unsafe {
            let level = if always_on_top {
                ffi::NSWindowLevel::NSFloatingWindowLevel
            } else {
                ffi::NSWindowLevel::NSNormalWindowLevel
            };
            let _: () = msg_send![*self.nswindow, setLevel:level];
        }
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<Icon>) {
        // macOS doesn't have window icons. Though, there is `setRepresentedFilename`, but that's
        // semantically distinct and should only be used when the window is in some way
        // representing a specific file/directory. For instance, Terminal.app uses this for the
        // CWD. Anyway, that should eventually be implemented as
        // `WindowBuilderExt::with_represented_file` or something, and doesn't have anything to do
        // with `set_window_icon`.
        // https://developer.apple.com/library/content/documentation/Cocoa/Conceptual/WinPanel/Tasks/SettingWindowTitle.html
    }

    #[inline]
    pub fn set_ime_spot(&self, logical_spot: LogicalPosition) {
        set_ime_spot(*self.nsview, *self.input_context, logical_spot.x, logical_spot.y);
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorHandle {
        unsafe {
            self::get_current_monitor(*self.nswindow)
        }
    }
}

impl WindowExtMacOS for UnownedWindow {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        *self.nswindow as *mut c_void
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        *self.nsview as *mut c_void
    }

    #[inline]
    fn request_user_attention(&self, is_critical: bool) {
        let request_type = if is_critical {
            NSRequestUserAttentionType::NSCriticalRequest
        } else {
            NSRequestUserAttentionType::NSInformationalRequest
        };

        unsafe {
            NSApp().requestUserAttention_(request_type);
        }
    }
}

impl Drop for UnownedWindow {
    fn drop(&mut self) {
        let id = self.id();
        self.window_list.access(|windows| windows.remove_window(id));

        // nswindow::close uses autorelease
        // so autorelease pool
        let autoreleasepool = unsafe { NSAutoreleasePool::new(nil) };

        // Close the window if it has not yet been closed.
        let nswindow = *self.nswindow;
        if nswindow != nil {
            let _: () = unsafe { msg_send![nswindow, close] };
        }

        let _: () = unsafe { msg_send![autoreleasepool, drain] };
    }
}

unsafe fn get_current_monitor(window: id) -> RootMonitorHandle {
    let screen: id = msg_send![window, screen];
    let desc = NSScreen::deviceDescription(screen);
    let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
    let value = NSDictionary::valueForKey_(desc, *key);
    let display_id = msg_send![value, unsignedIntegerValue];
    RootMonitorHandle { inner: EventLoop::make_monitor_from_display(display_id) }
}

unsafe fn set_min_dimensions<V: NSWindow + Copy>(window: V, mut min_size: LogicalSize) {
    let mut current_rect = NSWindow::frame(window);
    let content_rect = NSWindow::contentRectForFrameRect_(window, NSWindow::frame(window));
    // Convert from client area size to window size
    min_size.width += (current_rect.size.width - content_rect.size.width) as f64; // this tends to be 0
    min_size.height += (current_rect.size.height - content_rect.size.height) as f64;
    window.setMinSize_(NSSize {
        width: min_size.width as CGFloat,
        height: min_size.height as CGFloat,
    });
    // If necessary, resize the window to match constraint
    if current_rect.size.width < min_size.width {
        current_rect.size.width = min_size.width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height < min_size.height {
        // The origin point of a rectangle is at its bottom left in Cocoa.
        // To ensure the window's top-left point remains the same:
        current_rect.origin.y += current_rect.size.height - min_size.height;
        current_rect.size.height = min_size.height;
        window.setFrame_display_(current_rect, 0)
    }
}

unsafe fn set_max_dimensions<V: NSWindow + Copy>(window: V, mut max_size: LogicalSize) {
    let mut current_rect = NSWindow::frame(window);
    let content_rect = NSWindow::contentRectForFrameRect_(window, NSWindow::frame(window));
    // Convert from client area size to window size
    max_size.width += (current_rect.size.width - content_rect.size.width) as f64; // this tends to be 0
    max_size.height += (current_rect.size.height - content_rect.size.height) as f64;
    window.setMaxSize_(NSSize {
        width: max_size.width as CGFloat,
        height: max_size.height as CGFloat,
    });
    // If necessary, resize the window to match constraint
    if current_rect.size.width > max_size.width {
        current_rect.size.width = max_size.width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height > max_size.height {
        // The origin point of a rectangle is at its bottom left in Cocoa.
        // To ensure the window's top-left point remains the same:
        current_rect.origin.y += current_rect.size.height - max_size.height;
        current_rect.size.height = max_size.height;
        window.setFrame_display_(current_rect, 0)
    }
}

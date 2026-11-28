use objc2::{rc::Retained, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSScreen, NSView, NSWindow, NSWindowCollectionBehavior as Behavior,
    NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSDictionary, NSNumber, NSObject, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::CAMetalLayer;
use std::{
    cell::RefCell,
    ffi::c_void,
    sync::{mpsc, Arc},
};
use tauri::WebviewWindow;
use tss_live_wheel::wgpu;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGDisplayIsBuiltin(display: u32) -> u32;
    fn CGDisplayIsAsleep(display: u32) -> u32;
    fn CGDisplayIsOnline(display: u32) -> u32;
    fn CGDisplayBounds(display: u32) -> NSRect;
    fn CGWindowListCopyWindowInfo(options: u32, relative_to: u32) -> *mut c_void;
    fn CGRectMakeWithDictionaryRepresentation(bounds: *const c_void, result: *mut NSRect) -> bool;
}

struct AudienceWindow {
    stage: Retained<NSWindow>,
    editor: Retained<NSWindow>,
    original_behavior: Option<Behavior>,
    entering: bool,
}

thread_local! {
    static OUTPUT: RefCell<Option<AudienceWindow>> = const { RefCell::new(None) };
}

pub(super) struct Destination {
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) size: [u32; 2],
    pub(super) name: String,
    display_id: u32,
}

fn id(screen: &NSScreen) -> Option<u32> {
    screen
        .deviceDescription()
        .objectForKey(&NSString::from_str("NSScreenNumber"))?
        .downcast::<NSNumber>()
        .ok()
        .map(|n| n.unsignedIntValue())
}

fn builtin(screen: &NSScreen) -> bool {
    id(screen).is_some_and(|id| unsafe { CGDisplayIsBuiltin(id) != 0 })
}

fn pixel_size(screen: &NSScreen) -> [u32; 2] {
    let s = screen.frame().size;
    let k = screen.backingScaleFactor();
    [(s.width * k).round() as u32, (s.height * k).round() as u32]
}

// WindowServer's global coordinates use the main display's top-left origin.
// The Create Rule transfers a retained, toll-free-bridged NSArray to this scope.
fn on_screen_bounds(number: isize) -> Option<NSRect> {
    const ON_SCREEN_ONLY: u32 = 1;
    let windows = unsafe {
        Retained::<NSArray<NSDictionary<NSString, NSObject>>>::from_raw(
            CGWindowListCopyWindowInfo(ON_SCREEN_ONLY, 0).cast(),
        )
    }?;
    let number_key = NSString::from_str("kCGWindowNumber");
    let bounds_key = NSString::from_str("kCGWindowBounds");
    for window in windows.iter() {
        let Some(id) = window
            .objectForKey(&number_key)
            .and_then(|n| n.downcast::<NSNumber>().ok())
        else {
            continue;
        };
        if id.integerValue() != number {
            continue;
        }
        let bounds = window.objectForKey(&bounds_key)?;
        let mut rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(0., 0.));
        return unsafe {
            CGRectMakeWithDictionaryRepresentation(Retained::as_ptr(&bounds).cast(), &mut rect)
        }
        .then_some(rect);
    }
    None
}

fn fills_display(window: NSRect, display_id: u32) -> bool {
    let expected = unsafe { CGDisplayBounds(display_id) };
    [
        (window.origin.x, expected.origin.x),
        (window.origin.y, expected.origin.y),
        (window.size.width, expected.size.width),
        (window.size.height, expected.size.height),
    ]
    .iter()
    .all(|(a, b)| (a - b).abs() <= 2.)
}

pub(super) fn open(
    window: &WebviewWindow,
    instance: Arc<wgpu::Instance>,
) -> Result<Destination, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    window
        .with_webview(move |webview| {
            let result =
                (|| {
                    let mtm = MainThreadMarker::new()
                        .ok_or("Stereo displays require the AppKit thread")?;
                    let screens = NSScreen::screens(mtm);
                    let desk = screens
                        .iter()
                        .find(|s| builtin(s))
                        .ok_or("The internal performance display is unavailable")?;
                    let tv = screens.iter().filter(|s| !builtin(s))
                .max_by_key(|s| s.localizedName().to_string().to_lowercase().contains("samsung"))
                .ok_or("Stereo output needs a separate external display in extended desktop mode")?;
                    let display_id = id(&tv).ok_or("Cannot identify the external display")?;
                    let editor = unsafe { &*webview.inner().cast::<NSView>() }
                        .window()
                        .ok_or("Missing performance window")?;
                    if !editor.screen().is_some_and(|s| builtin(&s)) {
                        let work = desk.visibleFrame();
                        let old = editor.frame().size;
                        let size = NSSize::new(
                            old.width.min(work.size.width),
                            old.height.min(work.size.height),
                        );
                        let origin = NSPoint::new(
                            work.origin.x + (work.size.width - size.width) / 2.,
                            work.origin.y + (work.size.height - size.height) / 2.,
                        );
                        editor.setFrame_display(NSRect::new(origin, size), true);
                    }
                    let frame = tv.frame();
                    let stage = unsafe {
                        NSWindow::initWithContentRect_styleMask_backing_defer_screen(
                            NSWindow::alloc(mtm),
                            NSRect::new(NSPoint::new(0., 0.), frame.size),
                            NSWindowStyleMask::Titled
                                | NSWindowStyleMask::Resizable
                                | NSWindowStyleMask::FullSizeContentView,
                            NSBackingStoreType::Buffered,
                            false,
                            Some(&tv),
                        )
                    };
                    // The main-thread slot owns the NSWindow across render-thread presents.
                    unsafe { stage.setReleasedWhenClosed(false) };
                    stage.setFrame_display(frame, false);
                    stage.setTitle(&NSString::from_str("The Sacred Swirls • Live Stereo"));
                    stage.setTitleVisibility(NSWindowTitleVisibility::Hidden);
                    stage.setTitlebarAppearsTransparent(true);
                    stage.setCollectionBehavior(
                        Behavior::FullScreenPrimary | Behavior::FullScreenDisallowsTiling,
                    );
                    stage.setIgnoresMouseEvents(true);
                    stage.setHidesOnDeactivate(false);
                    stage.setHasShadow(false);
                    let view = NSView::initWithFrame(
                        NSView::alloc(mtm),
                        NSRect::new(NSPoint::new(0., 0.), frame.size),
                    );
                    let layer = CAMetalLayer::new();
                    layer.setContentsScale(tv.backingScaleFactor());
                    view.setLayer(Some(&layer));
                    view.setWantsLayer(true);
                    stage.setContentView(Some(&view));
                    // Metal retains the CAMetalLayer on the same adapter used for the editor.
                    let surface = unsafe {
                        instance.create_surface_unsafe(
                            wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                                Retained::as_ptr(&layer) as *mut c_void
                            ),
                        )
                    }
                    .map_err(|e| e.to_string())?;
                    let original_behavior =
                        (!NSScreen::screensHaveSeparateSpaces(mtm)).then(|| {
                            let previous = editor.collectionBehavior();
                            let conflicting = Behavior::FullScreenPrimary
                                | Behavior::FullScreenNone
                                | Behavior::Primary
                                | Behavior::CanJoinAllApplications;
                            editor.setCollectionBehavior(
                                (previous - conflicting)
                                    | Behavior::Auxiliary
                                    | Behavior::FullScreenAuxiliary,
                            );
                            previous
                        });
                    OUTPUT.with(|slot| {
                        *slot.borrow_mut() = Some(AudienceWindow {
                            stage,
                            editor,
                            original_behavior,
                            entering: true,
                        })
                    });
                    Ok(Destination {
                        surface,
                        size: pixel_size(&tv),
                        name: tv.localizedName().to_string(),
                        display_id,
                    })
                })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

pub(super) fn show(window: &WebviewWindow) -> Result<(), String> {
    window
        .run_on_main_thread(|| {
            OUTPUT.with(|slot| {
                if let Some(w) = slot.borrow().as_ref() {
                    w.stage.makeKeyAndOrderFront(None);
                    w.stage.toggleFullScreen(None);
                }
            })
        })
        .map_err(|e| e.to_string())
}

pub(super) fn close(window: &WebviewWindow) {
    let _ = window.run_on_main_thread(|| {
        OUTPUT.with(|slot| {
            if let Some(w) = slot.take() {
                w.stage.close();
                if let Some(previous) = w.original_behavior {
                    w.editor.setCollectionBehavior(previous);
                }
            }
        })
    });
}

impl Destination {
    pub(super) fn refresh(&mut self, window: &WebviewWindow) -> Result<bool, String> {
        let id_to_find = self.display_id;
        let (tx, rx) = mpsc::sync_channel(1);
        window
            .run_on_main_thread(move || {
                let placement = MainThreadMarker::new().and_then(|mtm| {
                    NSScreen::screens(mtm)
                        .iter()
                        .find(|s| id(s) == Some(id_to_find))
                        .map(|screen| {
                            let visible = OUTPUT.with(|slot| {
                                let mut state = slot.borrow_mut();
                                let w =
                                    state.as_mut().ok_or("The stereo audience window closed")?;
                                let visible = unsafe { CGDisplayIsAsleep(id_to_find) } == 0
                                    && w.stage.styleMask().contains(NSWindowStyleMask::FullScreen)
                                    && w.stage.screen().is_some_and(|s| id(&s) == Some(id_to_find))
                                    && on_screen_bounds(w.stage.windowNumber())
                                        .is_some_and(|rect| fills_display(rect, id_to_find));
                                // Restore input to the performance desk once, when TSS still owns focus.
                                // AppKit owns window placement throughout the fullscreen transition.
                                if w.entering && visible {
                                    w.entering = false;
                                    if w.stage.isKeyWindow() {
                                        w.editor.makeKeyAndOrderFront(None)
                                    }
                                }
                                Ok::<_, &'static str>(visible)
                            });
                            (pixel_size(&screen), visible)
                        })
                });
                let _ = tx.send(placement);
            })
            .map_err(|e| e.to_string())?;
        let placement = rx.recv().map_err(|e| e.to_string())?;
        if let Some((size, visible)) = placement {
            self.size = size;
            return visible.map_err(str::to_owned);
        }
        // AppKit can temporarily omit an online display during a reconfiguration.
        if unsafe { CGDisplayIsOnline(self.display_id) } != 0 {
            return Ok(false);
        }
        Err(format!(
            "The external stereo display {} disconnected.",
            self.name
        ))
    }
}

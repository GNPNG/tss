use objc2::{rc::Retained, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSMainMenuWindowLevel, NSScreen, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSNumber, NSPoint, NSRect, NSSize, NSString};
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
}

thread_local! {
    static OUTPUT: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
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
                        NSWindow::initWithContentRect_styleMask_backing_defer(
                            NSWindow::alloc(mtm),
                            frame,
                            NSWindowStyleMask::Borderless,
                            NSBackingStoreType::Buffered,
                            false,
                        )
                    };
                    // The main-thread slot owns the NSWindow across render-thread presents.
                    unsafe { stage.setReleasedWhenClosed(false) };
                    stage.setFrame_display(frame, false);
                    stage.setTitle(&NSString::from_str("The Sacred Swirls • Live Stereo"));
                    stage.setLevel(NSMainMenuWindowLevel + 1);
                    stage.setCollectionBehavior(
                        NSWindowCollectionBehavior::CanJoinAllSpaces
                            | NSWindowCollectionBehavior::FullScreenAuxiliary
                            | NSWindowCollectionBehavior::IgnoresCycle,
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
                    OUTPUT.with(|slot| *slot.borrow_mut() = Some(stage));
                    editor.makeKeyAndOrderFront(None);
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
                    w.orderFrontRegardless();
                }
            })
        })
        .map_err(|e| e.to_string())
}

pub(super) fn close(window: &WebviewWindow) {
    let _ = window.run_on_main_thread(|| {
        OUTPUT.with(|slot| {
            if let Some(w) = slot.take() {
                w.orderOut(None);
                w.close();
            }
        })
    });
}

impl Destination {
    pub(super) fn refresh(&mut self, window: &WebviewWindow) -> Result<(), String> {
        let id_to_find = self.display_id;
        let (tx, rx) = mpsc::sync_channel(1);
        window
            .run_on_main_thread(move || {
                let size = MainThreadMarker::new().and_then(|mtm| {
                    NSScreen::screens(mtm)
                        .iter()
                        .find(|s| id(s) == Some(id_to_find))
                        .map(|screen| {
                            OUTPUT.with(|slot| {
                                if let Some(w) = slot.borrow().as_ref() {
                                    let frame = screen.frame();
                                    if w.frame() != frame {
                                        w.setFrame_display(frame, true);
                                    }
                                }
                            });
                            pixel_size(&screen)
                        })
                });
                let _ = tx.send(size);
            })
            .map_err(|e| e.to_string())?;
        self.size = rx
            .recv()
            .map_err(|e| e.to_string())?
            .ok_or("The external stereo display disconnected")?;
        Ok(())
    }
}

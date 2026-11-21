use super::{Command, SharedSender};
use crate::live::{Phase, Rect};
use objc2::{define_class, msg_send, rc::Retained, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSResponder, NSView};
use objc2_foundation::{MainThreadMarker, NSObject, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;
use std::cell::Cell;
use std::ffi::c_void;
use tss_live_wheel::wgpu;

const TAG: isize = 0x5453_5357;

struct WheelViewState {
    sender: SharedSender,
    preview_hidden: Cell<bool>,
    export_hidden: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = WheelViewState]
    #[name = "TSSWheelPreviewView"]
    struct WheelView;

    impl WheelView {
        #[unsafe(method(tag))]
        fn view_tag(&self) -> isize { TAG }

        #[unsafe(method(mouseDown:))]
        fn down(&self, event: &NSEvent) { self.sample(Phase::Down, event); }
        #[unsafe(method(mouseDragged:))]
        fn drag(&self, event: &NSEvent) { self.sample(Phase::Move, event); }
        #[unsafe(method(mouseUp:))]
        fn up(&self, event: &NSEvent) { self.sample(Phase::Up, event); }
    }
);

impl WheelView {
    fn sample(&self, phase: Phase, event: &NSEvent) {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        let bounds = self.bounds().size;
        if bounds.width <= 0. || bounds.height <= 0. {
            return;
        }
        let point = [
            (p.x / bounds.width).clamp(0., 1.) as f32,
            (1. - p.y / bounds.height).clamp(0., 1.) as f32,
        ];
        if let Ok(sender) = self.ivars().sender.lock() {
            if let Some(tx) = &*sender {
                let _ = tx.send(Command::Pointer(phase, point));
            }
        }
    }
}

pub(super) fn layout(rect: Rect, scale: f64, view_height: f64, flipped: bool) -> NSRect {
    let (w, h) = (rect.width as f64 / scale, rect.height as f64 / scale);
    let top = rect.y as f64 / scale;
    NSRect::new(
        NSPoint::new(
            rect.x as f64 / scale,
            if flipped { top } else { view_height - top - h },
        ),
        NSSize::new(w, h),
    )
}

pub(super) fn child(parent: &NSView) -> Option<Retained<NSView>> {
    parent.viewWithTag(TAG)
}

pub(super) fn visibility(parent: &NSView, hidden: bool) {
    if let Some(view) = child(parent).and_then(|v| v.downcast::<WheelView>().ok()) {
        view.ivars().preview_hidden.set(hidden);
        view.setHidden(hidden || view.ivars().export_hidden.get());
    }
}

pub(super) fn cover_for_export(parent: &NSView, hidden: bool) {
    if let Some(view) = child(parent).and_then(|v| v.downcast::<WheelView>().ok()) {
        view.ivars().export_hidden.set(hidden);
        view.setHidden(hidden || view.ivars().preview_hidden.get());
    }
}

// This function only runs in Tauri's Cocoa main-thread webview callback.
pub(super) fn place(
    parent: &NSView,
    rect: Rect,
    sender: SharedSender,
    create_surface: bool,
    hidden: bool,
) -> Result<Option<(wgpu::Instance, wgpu::Surface<'static>)>, String> {
    let mtm = MainThreadMarker::new().ok_or("AppKit preview requires the main thread")?;
    let scale = parent
        .window()
        .ok_or("Preview webview has no window")?
        .backingScaleFactor();
    let frame = layout(rect, scale, parent.bounds().size.height, parent.isFlipped());
    let child = if let Some(existing) = parent.viewWithTag(TAG) {
        existing
    } else {
        let allocated = WheelView::alloc(mtm).set_ivars(WheelViewState {
            sender,
            preview_hidden: Cell::new(false),
            export_hidden: Cell::new(false),
        });
        // WheelView inherits NSView's designated initializer.
        let child: Retained<WheelView> =
            unsafe { msg_send![super(allocated), initWithFrame:frame] };
        let layer = CAMetalLayer::new();
        child.setLayer(Some(&layer));
        child.setWantsLayer(true);
        parent.addSubview(&child);
        Retained::into_super(child)
    };
    child.setFrame(frame);
    visibility(parent, hidden);
    let layer = child
        .layer()
        .ok_or("Preview view has no backing layer")?
        .downcast::<CAMetalLayer>()
        .map_err(|_| "Preview backing layer is not a CAMetalLayer")?;
    layer.setContentsScale(scale);
    if !create_surface {
        return Ok(None);
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });
    // The view hosts this CAMetalLayer. wgpu's Metal backend retains the layer
    // for the lifetime of the surface; the render thread never touches NSView.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
            Retained::as_ptr(&layer) as *mut c_void,
        ))
    }
    .map_err(|e| e.to_string())?;
    Ok(Some((instance, surface)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retinal_pixels_map_to_flipped_and_unflipped_webview_coordinates() {
        let r = Rect {
            x: 16,
            y: 40,
            width: 640,
            height: 360,
        };
        assert_eq!(
            layout(r, 2., 500., true),
            NSRect::new(NSPoint::new(8., 20.), NSSize::new(320., 180.))
        );
        assert_eq!(layout(r, 2., 500., false).origin, NSPoint::new(8., 300.));
        assert_eq!(layout(Rect { y: -10, ..r }, 1., 500., true).origin.y, -10.);
    }
}

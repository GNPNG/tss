# Gesture input experiment

For the Windows editor’s paired Safari performance surface, including slide assignments and renderer controls, see [Performance surfaces](../../docs/performance-surfaces.md). This experiment exercises the same event-capture module in isolation.

This is a standalone input diagnostic. It does not manipulate TSS's renderer, record or replay a project, provide a wireless connection, or constitute iPad support in the app. It captures WebKit Force Touch and standard pointer events into a device-neutral stream for future connection to the liquid simulation.

Serve the repository root locally and open the diagnostic in Safari on a Mac with Force Touch:

```sh
bun --hot experiments/gesture-input/serve.mjs
```

Open `http://127.0.0.1:47892/experiments/gesture-input/` and press and drag on the test surface. While holding the press, vary force and inspect `rawForce`, `pressureSource` and `pressure`. These values depend on hardware, browser, settings and actual event delivery. This experiment has not been tested with the user's hardware or the Tauri WebView. The server binds only localhost; an iPad cannot connect to it over the network. No device detection is inferred from the existence of an event property.

`createGestureInput(element, {onGesture, getTime, getLayerIds, sourceId, preventForceDefault})` emits `{time, sourceId, pointerId, pointerType, phase, x, y, pressure, pressureSource, rawForce, tiltX, tiltY, layerIds}`. `time` comes from `getTime(event)` in show seconds, or defaults to event timestamps in seconds. `x` and `y` are clamped normalized **displayed element** coordinates with a top-left origin; a letterboxed or cropped renderer must transform them to its simulation space. `layerIds` are copied at pointer-down and persist for that stroke. Phase is `down`, `move`, `up` or `cancel`; pointers are identified by source and pointer ID. `sourceId` defaults to `local` and should distinguish a future paired device. The caller must supply a shared show clock and establish ordering for remote events.

Mouse `PointerEvent.pressure` is ignored because its default 0.5 is not proof of a sensor. Pen/touch pointer pressure is passed through when present, but may also be a browser default and does not establish sensor support. A WebKit `webkitmouseforcechanged` event supplies `rawForce`; when WebKit exposes both mouse-down and force-click constants, `pressure` is `(rawForce - mouseDownThreshold) / (forceClickThreshold - mouseDownThreshold)`, clamped to 0–1. This is an adjustable-effect control range relative to those thresholds, not physical force or a calibration. If constants are unavailable, normalized pressure is null and raw force is retained. Preventing the default force-click action is scoped to this surface and can be disabled. Pressure on release is zero when a pressure sample has been received.

The target mapping is Apple Pencil movement to liquid swirl and Mac trackpad pressure to liquid-layer compression. These are planned renderer operations; capturing events alone does not implement them. Test concurrent devices, timing, coordinate transforms, pointer capture and real pressure on each supported device before treating them as verified. Apple Pencil properties can include estimated values at the native UIKit layer; this experiment does not receive or correct those native estimates.

This record format can represent concurrent streams when their source identities are available. The user specifically requested the MacBook's built-in trackpad and an external Bluetooth Magic Trackpad simultaneously. It does not establish that WebKit or macOS identifies those physical devices independently. WebKit force events here are associated with the active mouse pointer on the surface, so independently controlled pressure from both trackpads remains unverified and is not implemented.

Native macOS investigation has a documented starting point: [IOHIDManager](https://developer.apple.com/documentation/iokit/iohidmanager_h) enumerates HID devices, and [IOHIDDevice APIs](https://developer.apple.com/documentation/iokit/iohiddevicegetvaluewithcallback) include callbacks for a particular device. This establishes an interface for investigating per-device input, not that either trackpad exposes usable independent pressure data through that interface. Device reports, concurrent operation, system permissions and app integration remain untested.

Run the synthetic tests with Node 24 or later:

```sh
node --test experiments/gesture-input/gesture-input.test.mjs
```

References: [Apple WebKit Force Touch events](https://developer.apple.com/library/archive/documentation/AppleApplications/Conceptual/SafariJSProgTopics/RespondingtoForceTouchEventsfromJavaScript.html), [W3C Pointer Events](https://www.w3.org/TR/pointerevents3/), [WebKit's Apple Pencil event additions](https://webkit.org/blog/16301/webkit-features-in-safari-18-2/), [Apple Pencil native input](https://developer.apple.com/documentation/uikit/handling-input-from-apple-pencil).

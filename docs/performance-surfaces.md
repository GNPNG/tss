# Performance surfaces

Use Safari on a Mac as a Force Touch compression surface, or on an iPad as a Pencil swirl surface. The Windows editor receives the gestures and renders the selected Mathmos wheel. Several performers can join one show.

## Pair the first surface

1. Keep `tss-performer.exe` beside `the-sacred-swirls.exe`. Select a visible native Mathmos wheel in the editor and open **Performance surfaces**. The listener starts only when you press **Start listener**.
2. Enter the Windows machine’s LAN interface address under **Listen on IP**. The initial `127.0.0.1` accepts connections from the editor machine only. Enter the DNS name or IP address the performers will use under **Certificate hostname**. The name must resolve to that interface from each device and appear in the certificate’s subject alternative names. The default port is 47892. Port 0 chooses an available port.
3. Choose the PEM certificate chain and matching PEM private key on Windows. Browsers must already trust the issuing CA. For an internal CA, distribute its **public root certificate** through your usual device management process. On an iPad with a manually installed root, Apple documents the additional step under **Settings → General → About → Certificate Trust Settings**. Keep the server private key on Windows. [Apple’s iPad certificate instructions](https://support.apple.com/en-us/102390) · [macOS Keychain trust settings](https://support.apple.com/guide/keychain-access/change-the-trust-settings-of-a-certificate-kyca11871/mac)
4. Press **Start listener**, then **Copy pairing link**. Open it in Safari on each performance device. Enter a performer name and press **Connect**. The link contains a session secret. Share it privately with the performers. Each connection gets its own row in the editor.
5. In each row choose **Layer**, **Slide** and **Control**. Try the rear slide for the Mac and the front slide for the iPad. To operate the same slide together, assign one performer to compression and another to movement. If two contacts claim the same control on the same slide, the first one keeps it until release. Changing the assignment takes effect on the next contact.

## Perform

| Control | Action |
| --- | --- |
| Pencil swirl / trackpad pressure | Pencil and touch movement tug the material. On a compatible Mac, hold a click on the surface and vary trackpad force to compress the selected slide at the pointer. |
| Swirl material | Drag to tug the selected slide. An ordinary mouse can use this mode. |
| Rotate glass | Drag around the center to rotate the selected slide. |
| Compress | Pen or touch pressure, or WebKit trackpad force, controls local compression. Lift to release. |

The percentage on the surface is the input sent to the editor. For a trackpad, 0% corresponds to WebKit’s ordinary click threshold and 100% to its force-click threshold. A mouse’s conventional pressure value of 0.5 never drives compression. For Pencil and touch, the percentage comes from `PointerEvent.pressure`. Browser support for the event alone does not establish that the sensor is measuring variable force. [Apple’s Force Touch events](https://developer.apple.com/library/archive/documentation/AppleApplications/Conceptual/SafariJSProgTopics/RespondingtoForceTouchEventsfromJavaScript.html) · [W3C Pointer Events](https://www.w3.org/TR/pointerevents3/)

**Disconnect** releases that performer’s active controls. **Stop listener** releases all remote controls and invalidates the pairing link. If a device loses Wi-Fi while a press is held, the editor releases its controls when its session expires, approximately 15–16 seconds after the last accepted input or heartbeat. Closing the editor terminates the listener. Restart it for a fresh link.

## Verification and scope

Automated tests send synthetic WebKit and Pencil input through two independent sessions over real loopback HTTPS. They check the target layer and slide, a normalized 75% compression, a Pencil stroke, capture of the original target through reassignment, and release on disconnect. A Windows editor verification is available with `--verify-wheel-export --verify-performers` using the normal wheel-export verification environment. Its report records the commands received by the native gesture recorder.

Physical Apple Pencil pressure, Mac Force Touch delivery, end-to-end Wi-Fi latency and sustainable performer count still require measurement on the corresponding equipment. A Mac and iPad get independent identities when each joins the session. Safari can expose the internal and Bluetooth trackpads as a single mouse pointer. The web surface cannot assign those two trackpads independently unless the input API distinguishes them. When several mouse contacts are active and a force event lacks a pointer identifier, the surface discards that ambiguous event.

For a source checkout, `mise run dev` or `mise run build` builds the Go helper beside the Rust editor. A custom packager can run `go -C performer build -trimpath -o <absolute-output-file> .` using Go 1.23 or later. The helper embeds its HTML, CSS and JavaScript. To run the loopback transport and input regression tests on macOS or Linux with OpenSSL installed:

```sh
mise run performer:dev
go -C performer test ./...
TSS_PERFORMER_BIN="$PWD/src-tauri/target/debug/tss-performer" \
  mise x node@24.15.0 -- node --test performer/input-transport.test.mjs experiments/gesture-input/gesture-input.test.mjs
```

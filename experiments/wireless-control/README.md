# Wireless performance-surface experiment

The shipping Windows editor uses a compiled Go listener, with pairing and per-performer slide assignments on the deck. [Performance surfaces](../../docs/performance-surfaces.md)

This is an isolated HTTPS input and transport experiment. It is **not integrated with the TSS application or liquid renderer**. It does not yet change a visual, record a performance, synchronize device clocks, discover a laptop, pair devices through a graphical workflow, or provide a standalone iPad app. It has not been tested on an iPad, Apple Pencil, Tauri WebView, Windows, or an actual local network. The shared input module also supports other pointer devices; device-specific behavior must be tested on the actual device.

A browser receives a touch surface from the laptop, collects device-neutral gesture records from the shared `../gesture-input/gesture-input.mjs`, and posts them over HTTPS. A laptop callback receives batches in order, with a distinct server-assigned `sourceId` for each paired device; position, pointer type, pressure, tilt, and selected layer ID are preserved. The current CLI callback reports sample counts only. In future, the receiving renderer can map Pencil movement to swirl and simultaneous local trackpad pressure to compression. That behavior is not present here. Multiple remote controllers can pair concurrently, each with a separate server-issued session token and source identity. This preserves the distinction between simultaneous performers and a separate local laptop input source; control mappings and renderer arbitration are not implemented.

The timestamps are browser-local monotonic seconds; they are **not** the laptop's show clock. Deterministic replay will require explicit clock synchronization, ordering, recording, loss handling, and integration with the renderer. The sender batches input sequentially and stops with an error on rejection or queue overflow. It does not silently continue after a detected delivery failure. A failure can leave an incomplete gesture; resuming a live show safely needs a host-side cancellation policy.

## Run

Requires Node.js with the built-in test runner (the proof used Node 24), and a TLS private key and certificate chain already trusted by the iPad. The certificate must be valid for the chosen `PUBLIC_HOST`; the name must resolve to the laptop from the iPad. This experiment does not generate, install, or trust certificates and must not be used with TLS validation disabled.

From the TSS repository root, explicitly start the listener with an interface and address you intend to use:

```sh
mise x node@24.15.0 -- node experiments/wireless-control/server.mjs BIND_HOST PUBLIC_HOST PORT TLS_KEY TLS_CERT
```

For example, replace the placeholders with the laptop's intended local interface address, a matching trusted certificate hostname, a port, and the local paths to its TLS key and certificate. Open the printed HTTPS URL on the iPad, enter the randomly generated one-session secret displayed **once in the laptop terminal**, and draw in the surface. The Connect action verifies the secret and creates a device session; Disconnect revokes that session. Keep the pairing secret private. Stop the listener with Ctrl-C; it does not start automatically and the secret is regenerated on every launch. No gesture contents or session secret are written to logs by the server; the one-time pairing display is necessary to enter it on the iPad.

The server serves a fixed set of static files without pairing, and accepts pairing only with the random session secret and gesture submissions only with a distinct device session token, exact same HTTPS origin and host, and JSON content type. It bounds request size (16 KiB), batch size (32 records), aggregate rate (default 5,000 records/second), and concurrent sessions (default 1,024). The rate and session bounds are configurable through `TSS_GESTURE_RATE` and `TSS_MAX_SESSIONS` for the CLI; these are experiment resource limits, not TSS product limits. There is no fixed layer-count limit beyond the request size. It retains no session secret on disk. These controls are a research prototype, not a security audit or production pairing design.

## Verification

```sh
mise x node@24.15.0 -- node --test experiments/wireless-control/server.test.mjs
```

The local automated test creates an ephemeral self-signed test certificate, explicitly trusts it in the Node client, and exercises a real HTTPS request path. It checks trusted and untrusted certificates, origin and host rejection, missing/wrong secrets, content type, malformed and oversized messages, rate bounds, unpairing, distinct concurrent sessions, delivery of Pencil-shaped records, and more than sixteen layer IDs. The test certificate is not suitable for the iPad. Synthetic data verifies the transport contract only; it does not establish real Pencil pressure behavior, latency, wireless reliability, or visual control.

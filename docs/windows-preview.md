# Windows performance preview

Extract the Windows x64 portable ZIP and run `the-sacred-swirls.exe`. The native renderer uses the Windows Vulkan driver. The editor controls use the Microsoft Edge WebView2 Runtime. If the runtime is missing, install Microsoft's Evergreen Runtime. [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

1. Start on **Violet / Blue**. Pick **Rear slide**, set **Mouse action** to **Material tug**, and drag on the liquid. Switch to **Rotate slide** and drag around the center. Repeat on **Front slide**. **Projector focus** pulls between the two glasses.
2. Open an MP3 or M4A. Expand **Gesture macros**, start the song, choose **Record gesture** and drag. Choose **Finish recording**. Use **Play once** to audition. **At recording position** replays the gesture on the song timeline. Use **Trigger** to assign a repeating phrase, mute individual hits, or adjust the gain.
3. Use **Video feedback** to enable a recursive return and adjust delay, strength, zoom and rotation. **Projector** opens the audience picture. **Projector and audio outputs** sets the direct cue, audience route, presentation delay and audio trim. The audience audio starts off. Available separate audio devices depend on what Windows exposes to the installed WebView2 runtime.
4. With a song loaded and a live wheel selected, export the native wheel at 3840 × 2160 with the selected song and scheduled gestures. Automatic starting pose uses neutral slides for scheduled gestures and for both coral motion presets. Unscheduled **Wheel flow** uses its current pose. The existing **Export 4K preview** button renders the procedural track composition. [Native wheel guide](../live-wheel/README.md)

**Compress a slide.** Choose **Rear slide** or **Front slide**, then **Mouse action → Press slide**. Set **Mouse pressure**, press on a boundary between colors and move the contact point. Compression pushes oil sideways, changes the optical path through the film, and changes the focus locally. Release to restore the plate gap. The mouse strength is an assigned load. The other slide can continue its own motion underneath. [Pressure model and measured transport](https://github.com/GNPNG/tss/tree/main/experiments/pressure-study)

**Play a coral release.** In builds with the **Slide motion** selector, choose **Coral release** and pull **Projector focus** toward **Front slide** to see the droplets separate. The take lasts just under four seconds, then holds the last state. Shift focus toward **Rear slide** to see the folds behind the droplets. **Replay release live** starts the take again in the editor and on the projector at its configured delay. The seven wheel palettes work with either motion.

For smaller droplets with the original wheel behind them, select **Wheel with coral beads**. Watch the buds on the edge of the front film. **Replay front beads** restarts their release while the rear oil continues moving.

**Add a keyed or full-colour artwork track.** In builds with **Add artwork**, use that button on the timeline to import a clip, GIF or still. Open **Artwork settings** for key colour, cycling gradients, playback speed and Boolean masks. Choose **Export artwork program in 4K** to export the combined program and song. [Footage, alpha and mask controls](visual-media.md)

Use **Projected films** to see **Rear only**, **Front only**, **Both films** or **Clear glass** on the selected wheel. The hidden film keeps moving and retains its gestures. Changing the selection clears earlier video-feedback echoes. The audience sees the change at the configured projector delay. A native wheel export uses the film selection active when the export begins.

**Use a performance surface.** Keep `tss-performer.exe` beside the editor. Start the listener from the deck, pair the controller, then assign its visual track, slide and action. Two separate devices can perform together. The guide covers the TLS certificate, pairing link, sensor signals and tested configurations. [Performance surfaces](performance-surfaces.md)

**Control availability.** Select a visible live Mathmos wheel to show native controls. **Material response** becomes available in **Material tug** mode. Native export enables after the song has decoded and the preceding export has finished. Tick **Video feedback** to edit its return settings. **Choose audience device** appears when the WebView2 runtime supports the output picker. The route menus list the endpoints Windows exposes.

For MP4 export, install FFmpeg with the `libx264` and AAC encoders on the executable search path. If needed, run the following command in PowerShell, then relaunch the editor from a process that sees the new PATH. [WinGet package manifest](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/Gyan/FFmpeg)

```powershell
winget install --id Gyan.FFmpeg --exact
ffmpeg -version
```

The binary embeds its two independently simulated slides and seven wheel themes. The live optical resolution defaults to 640 × 360. The on-screen readout reports the selected adapter and actual presentation rate. The native MP4 is an 8-bit H.264/AAC deliverable. The offline wheel gallery includes 16-bit 4K Rec.709 stills. [Seven-wheel gallery](../experiments/slide-look-study/approval/wheels/README.md)


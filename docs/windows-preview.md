# The Sacred Swirls v0.1.0 RC quickstart

Extract the Windows x64 portable ZIP and run `the-sacred-swirls.exe`. The native renderer uses the Windows Vulkan driver. The editor controls use the Microsoft Edge WebView2 Runtime. If the runtime is missing, install Microsoft's Evergreen Runtime. [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

On Intel macOS, open `The Sacred Swirls.app`. The app uses Metal and the same project files. On either platform, expand **FFmpeg media tools** and choose **Recheck**. If tools are missing, **Install FFmpeg and FFprobe** uses the available package manager. The example needs both tools for artwork and FFmpeg with H.264 and AAC encoding for the movie.

## Open, perform and export Oil and Star

1. Choose **Open project** and select `examples/Oil and Star/Oil and Star.tss`. Keep `two-slide-cues.m4a`, `star-green.mp4` and `moving-matte.mov` in the same folder as the project. It finds the matching files there. Choose **Play** to hear the two-second cue and see the keyed star over the oil. The hidden matte track shapes the star.
2. In **TRACKS**, select **Violet / Blue**. Under **Slide motion**, choose **Recorded slide motion**. Change **Front film oil** from **1,000 cSt PDMS** to **20 cSt PDMS**, leaving **Rear film oil** at **Recorded oil motion**. To inspect the front film, set **Projected films** to **Front only**. Restore **Both films** before exporting. To manipulate the front glass live, set **Mouse target** to **Front slide**, choose **Material tug** or **Rotate slide** under **Mouse action**, and drag on the liquid.
3. On Windows, expand **Gesture macros**. To audition an existing take, choose **Pull the rear oil** under **Saved gesture** and click **Play once**. To record your own timing, click **Return to start**, **Play**, then **Record gesture**. Drag on the liquid, release, and click **Finish recording**. Leave **Macro on** checked and **Trigger** at **At recording position** to include the take in playback and export. On Mac, the native movie export includes the scheduled gestures saved in the project. The live gesture recorder and **Play once** controls are Windows features.
4. Choose **Save project** to keep your oil and performance edits in a new `.tss` beside the three media files. To make the movie, leave **Export starting pose** on **Automatic · neutral for releases or scheduled gestures**, click **Export artwork program in 4K**, and choose an MP4 filename. Both desktop builds output the composed artwork, oil and source audio at 3840 × 2160, 30 fps, with 8-bit H.264 video and AAC audio.

If a file has moved, **Relink audio** locates the original song. For a missing clip or matte, select its track, expand **Artwork settings** and choose **Relink source**. Relinking verifies the file's contents.

For a 16-bit still on Windows, select **Violet / Blue**, expand **Selected wheel · production stills**, choose **4K visualizer · layered** and click **Export selected wheel · 16-bit stills**. This writes PNGs of the selected wheel and its individual films. The star and matte belong to the composed MP4. [Native wheel guide](../live-wheel/README.md)

## Windows performance controls

**Projector** opens the audience picture. **Projector and audio outputs** sets the direct cue, audience route, presentation delay and audio trim. The audience audio starts off. Available separate audio devices depend on what Windows exposes to the installed WebView2 runtime. **Video feedback** controls the recursive return delay, strength, zoom and rotation.

**Compress a slide.** Choose **Rear slide** or **Front slide**, then **Mouse action → Press slide**. Set **Mouse pressure**, press on a boundary between colors and move the contact point. Compression pushes oil sideways, changes the optical path through the film, and changes the focus locally. Release to restore the plate gap. The mouse strength is an assigned load. The other slide can continue its own motion underneath. [Pressure model and measured transport](https://github.com/GNPNG/tss/tree/main/experiments/pressure-study)

**Play a coral release.** In builds with the **Slide motion** selector, choose **Coral release** and pull **Projector focus** toward **Front slide** to see the droplets separate. The take lasts just under four seconds, then holds the last state. Shift focus toward **Rear slide** to see the folds behind the droplets. **Replay release live** starts the take again in the editor and on the projector at its configured delay. The seven wheel palettes work with either motion.

For smaller droplets with the original slide behind them, select **Recorded slide with coral beads**. Watch the buds on the edge of the front film. **Replay front beads** restarts their release while the rear oil continues moving.

**Add a keyed or full-colour artwork track.** In builds with **Add artwork**, use that button on the timeline to import a clip, GIF or still. Open **Artwork settings** for key colour, cycling gradients, playback speed and Boolean masks. Choose **Export artwork program in 4K** to export the combined program and song. [Footage, alpha and mask controls](visual-media.md)

Use **Projected films** to see **Rear only**, **Front only**, **Both films** or **Clear glass** on the selected wheel. The hidden film keeps moving and retains its gestures. Changing the selection clears earlier video-feedback echoes. The audience sees the change at the configured projector delay. A native wheel export uses the film selection active when the export begins.

**Use a performance surface.** Keep `tss-performer.exe` beside the editor. Start the listener from the deck, pair the controller, then assign its visual track, slide and action. Two separate devices can perform together. The guide covers the TLS certificate, pairing link, sensor signals and tested configurations. [Performance surfaces](performance-surfaces.md)

**Control availability.** Select a visible live slide to show native controls. **Material response** becomes available in **Material tug** mode. Native export enables after the song has decoded and the preceding export has finished. Tick **Video feedback** to edit its return settings. **Choose audience device** appears when the WebView2 runtime supports the output picker. The route menus list the endpoints Windows exposes.

For MP4 export, install FFmpeg with the `libx264` and AAC encoders on the executable search path. If needed, run the following command in PowerShell, then relaunch the editor from a process that sees the new PATH. [WinGet package manifest](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/Gyan/FFmpeg)

```powershell
winget install --id Gyan.FFmpeg --exact
ffmpeg -version
```

The binary embeds its two independently simulated slides and seven wheel themes. The live optical resolution defaults to 640 × 360. The on-screen readout reports the selected adapter and actual presentation rate. The native MP4 is an 8-bit H.264/AAC deliverable. The offline wheel gallery includes 16-bit 4K Rec.709 stills. [Seven-wheel gallery](../experiments/slide-look-study/approval/wheels/README.md)


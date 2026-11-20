# Windows performance preview

Extract the Windows x64 portable ZIP and run `the-sacred-swirls.exe`. The native renderer uses the Windows Vulkan driver. The editor controls use the Microsoft Edge WebView2 Runtime. If the runtime is missing, install Microsoft's Evergreen Runtime. [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

1. Start on **Violet / Blue**. Pick **Rear slide**, set **Mouse action** to **Material tug**, and drag on the liquid. Switch to **Rotate slide** and drag around the center. Repeat on **Front slide**. **Projector focus** pulls between the two glasses.
2. Open an MP3 or M4A. Expand **Gesture macros**, start the song, choose **Record gesture** and drag. Choose **Finish recording**. Use **Play once** to audition. **At recording position** replays the gesture on the song timeline. Use **Trigger** to assign a repeating phrase, mute individual hits, or adjust the gain.
3. Use **Video feedback** to enable a recursive return and adjust delay, strength, zoom and rotation. **Projector** opens the audience picture. **Projector and audio outputs** sets the direct cue, audience route, presentation delay and audio trim. The audience audio starts off. Available separate audio devices depend on what Windows exposes to the installed WebView2 runtime.
4. With a song loaded and a live wheel selected, export the native wheel at 3840 × 2160 with the selected song and scheduled gestures. Automatic starting pose uses neutral slides for scheduled gestures and the current pose for an unprogrammed wheel. The existing **Export 4K preview** button renders the procedural track composition. [Native wheel guide](../live-wheel/README.md)

For MP4 export, install FFmpeg with the `libx264` and AAC encoders on the executable search path. If needed, run the following command in PowerShell, then relaunch the editor from a process that sees the new PATH. [WinGet package manifest](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/Gyan/FFmpeg)

```powershell
winget install --id Gyan.FFmpeg --exact
ffmpeg -version
```

The binary embeds its two independently simulated slides and seven wheel themes. The live optical resolution defaults to 640 × 360. The on-screen readout reports the selected adapter and actual presentation rate. The native MP4 is an 8-bit H.264/AAC deliverable. The offline wheel gallery includes 16-bit 4K Rec.709 stills. [Seven-wheel gallery](../experiments/slide-look-study/approval/wheels/README.md)


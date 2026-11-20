# Projector output and cue audio

Choose **Projector** on a native Windows wheel to open a separate program window. Use **Projector and audio outputs** for the intentional delay, direct DJ cue output, audience output and audio trim. The audience route starts muted. Choosing the same endpoint for both routes gives that endpoint to the audience. Headphones and a separate program endpoint allow both signals at once. **Choose audience device** is available when the embedded runtime exposes its output-device picker.

The cue is a direct branch of the decoded song. The second branch runs through a Web Audio delay line, a media-stream destination and a media element bound to the selected device. The projector delays completed program frames. The video feedback effect reads recursively from an independently timed, shorter program history. Start at 500 ms projector delay and 100 ms feedback delay to hear a cue now and see its echoed gesture inside the delayed audience picture. Audience audio trim moves the digital audio delay independently to compensate measured device latency.

An impulse placed at sample 32 appeared in the cue at sample 32 and in the 500 ms audience signal at sample 24,032 with 48 kHz audio. Eight offline Web Audio cases covered 44.1 and 48 kHz, zero delay, 500 ms, a negative calibration trim and the largest supported audio delay. Each measured offset matched the requested sample count. Hardware device queues and projector display latency require measurement at the venue.

[Audio impulse measurements](audio-impulse.json) · [Repeatable signal test](audio-impulse.mjs) · [W3C Web Audio delay node](https://www.w3.org/TR/webaudio-1.0/#DelayNode) · [W3C audio output selection](https://www.w3.org/TR/audio-output/) · [Chromium audio output routing](https://developer.chrome.com/blog/audiocontext-setsinkid/)


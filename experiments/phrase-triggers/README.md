# Gesture macros and phrase triggers

On Windows, select a live wheel, expand **Gesture macros**, and click **Record gesture**. Drag the chosen slide in the native viewport and click **Finish recording**. **Play once** auditions the take on its recorded visual track. The recording includes lead-in, lift and trailing time. Open a song before recording to associate the take with its audio-content fingerprint. Windows stores the takes locally.

Choose a recurring lyric or acoustic cue in **Trigger**. **Fit gesture to phrase** stretches the event times to the actual matched interval. For occurrence index *i*, starting at zero, the replay gain is `min(maximum, amount + i × extra)`. For a tug this scales displacement about the start point. For a rotation it scales the unwrapped turn angle about the slide center. **Macro on** arms the cue. Each occurrence can be muted and auditioned from its timeline position. Operators can run separate macros on the rear and front concurrently. A new take targeting an occupied slide replaces that slide's prior take. The current native viewport accepts the selected visual track.

Audio import offers repeated four-transient spectral/rhythm signatures from the 30 Hz prescan. It also checks for same-basename VTT, SRT and local-transcribe JSON beside the audio, in that order. The **Timed lyrics** control can select a different file. Word timestamps set precise phrase bounds when available. A phrase inside a subtitle cue uses that cue's supplied interval. The visual macros replay captured pointer input. The physics trajectory underneath the performance uses the editor's buffered slide clock.

A take recorded while the song is playing defaults to **At recording position**. On the next playback it starts at the captured transport position. Recording while paused produces a manual take that can be assigned to a recurring cue.

The separate C++ fixture compiles two Vectorscan literals, one sequence of normalized timed words and one sequence of supplied acoustic tokens. It checks the match offsets against exact song-sample intervals. The acoustic fixture supplies its three event labels. The integrated WebView detector takes PCM-derived envelopes. Vectorscan is a candidate for applying larger predeclared pattern banks.

```sh
c++ -std=c++17 -O2 experiments/phrase-triggers/match.cpp $(pkg-config --cflags --libs libhs) -o /tmp/tss-phrase-match
/tmp/tss-phrase-match
node --test experiments/phrase-cues/*.test.mjs
```

[Measured sample offsets](fixture.csv) · [Native fixture](match.cpp) · [Editor detector](../../WebAssets/phrase-cues.js) · [Native replay lifecycle test](../phrase-cues/gesture-macros.test.mjs)


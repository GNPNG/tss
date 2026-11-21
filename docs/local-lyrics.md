# Local lyric transcription

Open a song with **Open audio**, expand **Local lyric transcription**, then choose **Transcribe song**. TSS runs the GNPNG local-transcribe installation on this computer. The tool receives the selected recording and writes timed English segments into a private temporary directory. You can cancel an active transcription. Selecting another recording discards the old draft.

Edit words and cue boundaries in the WebVTT draft. **Replace lyric triggers** sends reviewed, repeated phrases to the Windows gesture macro deck for the current song. Existing recordings and acoustic trigger groups stay available. **Save WebVTT** opens a file picker on both platforms. A VTT with the same basename as the audio is available to the editor's timed-lyric sidecar reader. Choosing a new filename also lets you keep alternative transcriptions.

## Installation

The optional dependency is [GNPNG local-transcribe](https://gitea.gnomatix.com/gnpng/local-transcribe). Its installer places the command, whisper.cpp binary and speech model together. Follow the project's platform preflight and run its sample check.

- On macOS run `./install.sh --test` from the local-transcribe checkout. TSS invokes the installed wrapper with system Bash. It searches the desktop process's PATH and the standard user-local and Homebrew bin directories.
- On Windows run `pwsh .\install.ps1 -Test` from that checkout. The editor invokes PowerShell 7 and the installed `local-transcribe.ps1`. FFmpeg is required for M4A.
- The editor requests CPU decoding, so transcription can run alongside the live Windows renderer. The Intel Mac installation can also use its Core ML encoder. The upstream command currently selects English, and recognition quality on singing varies with the mix. Review the proposed words and times before applying them.

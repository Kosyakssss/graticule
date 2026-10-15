# The audio service

`crates/audio` plays and records sound for an app, through `cpal`, which reaches each system's audio devices (Core Audio, WASAPI, ALSA) without `unsafe` in our code. Four-track is its first user.

## How the facts reach the audio thread

The app works out a `Mix` from its facts as a derived value: every voice (a sound, where it starts, which part of it, at what gain) and the transport (stopped, playing from a time, recording a take). `Jobs::serve` hands the engine the reading end of that derived value, a `Watch`, and the writing end of a live value, the `Deck`. The view never talks to the engine and the engine never touches the facts; the derived value is the only way in and the live value the only way out, so the app stays a pure function of its facts and Assay, Specimen and replays run it with no engine at all.

`Watch::read` loads the newest `Mix` through `arc-swap` without a lock or a reference count, so the output callback reads it every time it runs. A new press of Play is told apart from the last by the transport's `cue`, a number the command draws at random; the callback restarts from the transport's time when the cue changes.

## Idle costs nothing

While stopped, the output stream is paused and the engine's own thread blocks in `Watch::wait` until the mix changes. While playing it wakes every 16 ms to publish where it is, which wakes the window at most once a frame. Nothing runs when nothing plays.

## Traps

- The output callback never allocates after its first call: its scratch buffer grows once to the device's buffer size. The input callback does allocate, a `Vec` a buffer, sent over a channel to the engine's thread; a lock-free ring would avoid it, and is worth it if a device ever drops input under load.
- Every sound is one channel at 48 kHz, the mix's time is counted in those samples, and a device at another rate is fed by stepping through the mix at the ratio of the two rates. Files at other rates are brought to 48 kHz by straight lines between neighbouring samples when they are read, which is fine for listening and not for mastering.
- A recorded take's clip is added by a command, not by the engine. Stop starts a job that waits (up to ten seconds) for the engine to write the take to the app's folder (`saved`) and replies with its length; the reply adds the clip. So the command log holds an ordinary insertion, and a replay never depends on a sound card. With no engine running, `saved` answers at once that none is.
- A take is written as 32-bit floating point WAV under `takes/` in the app's folder, named by the take's number. Takes that no clip uses any more are kept, since undo can bring their clips back.
- A waveform is drawn as one filled path per clip, four steps to a column. Drawing it as a rectangle per step put the slowest 1% of frames over budget; one path keeps them near 0.6 ms.

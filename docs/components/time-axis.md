# Time axis

A ruler over tracks: what happens when, laid out across in time and down in tracks, with a playhead. The bones of a sequencer, a video editor, a log of events or a chart of trades.

## Use it for

Anything placed in time that the user moves through, zooms into and edits: clips on tracks, notes, spans of a recording, events from several sources side by side.

## Code

```rust
#[session]
view: TimeView,
#[session]
playhead: i64,

ui.time_axis(Clock::Beats { per_beat: 96, beats_per_bar: 4 }, Song::view, Song::playhead, |axis| {
    for track in &self.tracks {
        axis.track(&track.name, 2, |lane| {
            for clip in track.clips.iter().filter(|c| lane.shows(c.start, c.end)) {
                lane.span(clip.start, clip.end).ink(Ink::Selection).fill();
                lane.label(clip.start, &clip.name);
            }
        });
    }
});
```

Time is a whole number in the app's own unit: samples, milliseconds, ticks. `TimeView` is the stretch that shows, the time at the left edge and how much each column holds; scrolling and zooming change it, so keep it a session fact. `Clock` says how the time is counted: `Plain` numbers, `Seconds` or bars and `Beats`. The ruler's marks fall where the clock says, labelled at least ten columns apart, with smaller marks between; labels sit on whole cells.

A lane is a canvas whose space runs across in the axis's time and down from 0 to 1, so anything can be drawn on it. `span` draws a box between two times, `label` writes at a time, and `press` names a shape that runs a command, as on any canvas. Draw only what `shows` says is in view, so a long piece costs what a screenful does.

## Mouse

A click on the ruler moves the playhead there, snapped to the nearest small mark; dragging shows where it will land and moves it when let go. The wheel with the primary key held zooms around the pointer; with Shift held, or sideways on a trackpad, it scrolls.

## Keys

Tab reaches the ruler. Left and Right move the playhead by a small mark, Page Up and Page Down by a labelled one, and the view follows it.

## Narrow windows

Track names keep sixteen columns and the lanes take the rest; a narrower window shows less time at the same zoom.

## Screen reader

The ruler is a slider named "Playhead" whose value is the playhead's time as the clock says it. Each lane is a canvas named for its track.

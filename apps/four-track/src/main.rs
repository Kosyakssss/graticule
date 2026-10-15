mod band;

use std::collections::HashMap;
use std::sync::Arc;

use audio::{Deck, Mix, RATE, Sound, Transport, Voice};
use band::{BAR, BEAT, LOOP, Part};
use graticule::kernel::Drag;
use graticule::kernel::canvas::Weight;
use graticule::kernel::time::{Clock, TimeView};
use graticule::ledger::Choose;
use graticule::prelude::*;

#[derive(Facts)]
struct FourTrack {
    tracks: List<Track>,
    clips: List<Clip>,
    #[session]
    view: TimeView,
    #[session]
    playhead: i64,
    #[session]
    rolling: Rolling,
    #[session]
    snap: Snap,
    #[session]
    chosen: Selection<Clip>,
    #[session]
    track_chosen: Selection<Track>,
    #[derived(Sounds::of, from = [clips, sounds])]
    sounds: Derived<Sounds>,
    #[derived(FourTrack::mixed, from = [clips, tracks, sounds, rolling])]
    mix: Derived<Mix>,
    #[live]
    deck: Live<Deck>,
}

/// One of the four tracks: its name, and whether it is heard.
#[derive(Facts)]
struct Track {
    #[key]
    id: Key<Track>,
    name: String,
    muted: bool,
}

/// A stretch of a sound on a track: from sample `from` of the sound, `len` samples long, starting at `at`.
#[derive(Facts)]
struct Clip {
    #[key]
    id: Key<Clip>,
    track: Key<Track>,
    sound: Source,
    at: i64,
    from: i64,
    len: i64,
}

impl Clip {
    fn end(&self) -> i64 {
        self.at + self.len
    }
}

/// Where a clip's sound comes from: one of the band's loops, or a take recorded from the input.
#[derive(Facts, Copy, Debug, PartialEq, Eq, Hash)]
enum Source {
    Band(Part),
    Take(u64),
}

impl Default for Source {
    fn default() -> Self {
        Source::Band(Part::Drums)
    }
}

/// What the transport is doing: stopped, playing from a time, or recording a take on a track from a time. The number tells one press of Play from the next.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Rolling {
    #[default]
    Stopped,
    Playing(i64, u64),
    Recording(i64, u64, u64, Key<Track>),
}

/// What moving a clip or the playhead lands on.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq)]
enum Snap {
    #[label("Bars")]
    Bar,
    #[default]
    #[label("Beats")]
    Beat,
    #[label("Off")]
    Off,
}

impl Snap {
    fn to(self, t: i64) -> i64 {
        let step = match self {
            Snap::Bar => BAR,
            Snap::Beat => BEAT,
            Snap::Off => 1,
        };
        (t as f64 / step as f64).round() as i64 * step
    }
}

/// Every sound the clips use, read off the main thread: the band's loops, and takes read from their files, each kept once read.
#[derive(Default)]
struct Sounds(HashMap<Source, Result<Arc<Sound>, String>>);

impl Sounds {
    fn of(clips: &List<Clip>, last: &Derived<Sounds>) -> Sounds {
        let mut found = HashMap::new();
        for clip in clips.iter() {
            if found.contains_key(&clip.sound) {
                continue;
            }
            let kept = last.get().and_then(|l| l.0.get(&clip.sound)).filter(|s| s.is_ok()).cloned();
            let sound = kept.unwrap_or_else(|| match clip.sound {
                Source::Band(part) => Ok(part.sound()),
                Source::Take(take) => std::fs::read(audio::take_path(&folder(), take)).map_err(|e| e.to_string()).and_then(|b| Sound::read_wav(&b)).map(Arc::new),
            });
            found.insert(clip.sound, sound);
        }
        Sounds(found)
    }

    fn get(&self, source: Source) -> Option<&Arc<Sound>> {
        self.0.get(&source).and_then(|s| s.as_ref().ok())
    }
}

/// Where takes are kept: the app's own folder.
fn folder() -> std::path::PathBuf {
    graticule::kernel::runner::folder()
}

#[derive(Commands)]
enum Cmd {
    #[label("Play")]
    Play,
    #[label("Stop")]
    Stop,
    #[label("Record")]
    Record,
    #[label("Go to the start")]
    ToStart,
    #[label("Add a drum loop", short: "Drums")]
    AddDrums,
    #[label("Add a bass loop", short: "Bass")]
    AddBass,
    #[label("Add a keys loop", short: "Keys")]
    AddKeys,
    #[label("Add a lead loop", short: "Lead")]
    AddLead,
    #[label("Repeat the chosen clips", short: "Repeat")]
    Repeat,
    #[label("Split the chosen clips at the playhead", short: "Split")]
    Split,
    #[label("Remove the clip", short: "Remove", many: "Remove {count} clips")]
    Remove(#[subject] Key<Clip>),
    #[label("Mute the track", short: "Mute")]
    Mute(#[subject] Key<Track>),
    #[label("Unmute the track", short: "Unmute")]
    Unmute(#[subject] Key<Track>),
    #[label("Zoom in")]
    ZoomIn,
    #[label("Zoom out")]
    ZoomOut,
    #[label("Bounce the song to a file…", short: "Bounce…")]
    Bounce,
    #[internal]
    Bounced(Result<Path, Failure>),
    #[internal]
    Recorded(Key<Track>, i64, u64, Result<i64, Failure>),
    #[internal]
    Dragged(Key<Track>, Drag),
    #[internal]
    Edit(Edit),
}

impl App for FourTrack {
    type Cmd = Cmd;

    fn start(&self, jobs: &mut Jobs<Cmd>) {
        jobs.serve(t!("Audio"), audio::run, FourTrack::mix, FourTrack::deck);
    }

    fn available(&self, cmd: &Cmd) -> Available {
        let stopped = self.rolling == Rolling::Stopped;
        match cmd {
            Cmd::Play => match self.rolling {
                Rolling::Stopped => no_if(self.clips.is_empty(), t!("There is nothing to play")),
                _ => No(t!("Playing already")),
            },
            Cmd::Stop => no_if(stopped, t!("Stopped already")),
            Cmd::Record => match (self.rolling, self.track_chosen.focused()) {
                (Rolling::Recording(..), _) => No(t!("Recording already")),
                (_, None) => No(t!("Choose a track to record on")),
                _ => Yes,
            },
            Cmd::ToStart => {
                let at = if stopped { self.playhead } else { self.deck.now().at };
                no_if(self.playhead == 0 && at == 0, t!("At the start already"))
            }
            Cmd::AddDrums | Cmd::AddBass | Cmd::AddKeys | Cmd::AddLead => no_if(self.track_chosen.focused().is_none(), t!("Choose a track to add it to")),
            Cmd::Repeat => no_if(self.chosen.is_empty(), t!("Choose a clip first")),
            Cmd::Split => match self.chosen.is_empty() {
                true => No(t!("Choose a clip first")),
                false => no_if(!self.chosen.iter().filter_map(|k| self.clips.find(k)).any(|c| c.at < self.playhead && self.playhead < c.end()), t!("The playhead isn't inside a chosen clip")),
            },
            Cmd::Remove(c) => no_if(!self.clips.has(*c), t!("That clip is gone")),
            Cmd::Mute(t) => match self.tracks.find(*t) {
                Some(track) => no_if(track.muted, t!("Muted already")),
                None => No(t!("That track is gone")),
            },
            Cmd::Unmute(t) => match self.tracks.find(*t) {
                Some(track) => no_if(!track.muted, t!("Not muted")),
                None => No(t!("That track is gone")),
            },
            Cmd::ZoomIn => no_if(self.view.per_col <= 16, t!("As near as it goes")),
            Cmd::ZoomOut => no_if(self.view.per_col >= RATE as i64 * 4, t!("As far as it goes")),
            Cmd::Bounce => match self.clips.is_empty() {
                true => No(t!("There is nothing to bounce")),
                false => no_if(self.mix.get().is_none(), t!("The mix is still being worked out")),
            },
            Cmd::Bounced(_) | Cmd::Recorded(..) | Cmd::Dragged(..) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Play => {
                self.rolling = Rolling::Playing(self.playhead, cx.random());
                Outcome::Changed
            }
            Cmd::Stop => {
                let deck = self.deck.now();
                if let Rolling::Recording(from, _, take, track) = self.rolling {
                    cx.job(t!("Saving the take"), move || (track, from, take, audio::saved(take).map_err(Failure::new)), |(track, at, take, len)| Cmd::Recorded(track, at, take, len));
                }
                if deck.playing {
                    self.playhead = deck.at.max(0);
                }
                self.rolling = Rolling::Stopped;
                Outcome::Changed
            }
            Cmd::Record => {
                let Some(track) = self.track_chosen.focused() else { return Outcome::Nothing };
                self.rolling = Rolling::Recording(self.playhead, cx.random(), cx.random(), track);
                Outcome::Changed
            }
            Cmd::ToStart => {
                self.playhead = 0;
                self.view = self.view.showing(0, 1);
                if self.rolling != Rolling::Stopped {
                    self.rolling = Rolling::Playing(0, cx.random());
                }
                Outcome::Changed
            }
            Cmd::AddDrums => self.add(Part::Drums, cx),
            Cmd::AddBass => self.add(Part::Bass, cx),
            Cmd::AddKeys => self.add(Part::Keys, cx),
            Cmd::AddLead => self.add(Part::Lead, cx),
            Cmd::Repeat => {
                let chosen: Vec<_> = self.chosen.iter().filter_map(|k| self.clips.find(k)).map(|c| (c.track, c.sound, c.end(), c.from, c.len)).collect();
                for (track, sound, at, from, len) in chosen {
                    let _ = self.clips.push(Clip { id: cx.key(), track, sound, at, from, len });
                }
                Outcome::Changed
            }
            Cmd::Split => {
                let at = self.playhead;
                let inside: Vec<_> =
                    self.chosen.iter().filter_map(|k| self.clips.find(k)).filter(|c| c.at < at && at < c.end()).map(|c| (c.id, c.track, c.sound, c.from + (at - c.at), c.end() - at)).collect();
                for (id, track, sound, from, len) in inside {
                    let _ = self.clips.change(id, |c| c.len = at - c.at);
                    let _ = self.clips.push(Clip { id: cx.key(), track, sound, at, from, len });
                }
                Outcome::Changed
            }
            Cmd::Remove(c) => self.clips.remove(c),
            Cmd::Mute(t) => self.tracks.change(t, |t| t.muted = true),
            Cmd::Unmute(t) => self.tracks.change(t, |t| t.muted = false),
            Cmd::ZoomIn => self.zoom(true),
            Cmd::ZoomOut => self.zoom(false),
            Cmd::Bounce => {
                cx.save_as("Song.wav", bounce, Cmd::Bounced);
                Outcome::Nothing
            }
            Cmd::Bounced(Ok(_)) => Outcome::Nothing,
            Cmd::Bounced(Err(failure)) => cx.say_failure(failure),
            Cmd::Recorded(track, at, take, len) => match len {
                Ok(len) if len > 0 => self.clips.push(Clip { id: cx.key(), track, sound: Source::Take(take), at, from: 0, len }),
                Ok(_) => cx.say(t!("Nothing was heard from the input")),
                Err(failure) => cx.say_failure(failure),
            },
            Cmd::Dragged(track, drag) => self.dragged(track, drag),
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        let deck = self.deck.now();
        ui.page(t!("Four-track"), |ui| {
            ui.row(|ui| {
                match self.rolling {
                    Rolling::Stopped => {
                        ui.button(Cmd::Play);
                    }
                    _ => {
                        ui.button(Cmd::Stop);
                    }
                }
                ui.button(Cmd::Record);
                ui.button(Cmd::ToStart);
                ui.button(Cmd::ZoomIn);
                ui.button(Cmd::ZoomOut);
                ui.choice(t!("Snap to"), FourTrack::snap);
                ui.button(Cmd::Bounce);
            });
            let now = if deck.playing && self.rolling != Rolling::Stopped { deck.at } else { self.playhead };
            let said = match self.rolling {
                Rolling::Stopped => {
                    let at = clock(self.playhead);
                    t!("Stopped at {at}")
                }
                Rolling::Playing(..) => {
                    let at = clock(now);
                    t!("Playing, {at}")
                }
                Rolling::Recording(.., track) => {
                    let name = self.tracks.find(track).map_or(String::new(), |t| t.name.clone());
                    let at = clock(now);
                    t!("Recording on {name}, {at}")
                }
            };
            ui.row(|ui| {
                ui.text(said);
                match &deck.device {
                    Ok(name) => {
                        let name = name.clone();
                        ui.muted(t!("through {name}"));
                    }
                    Err(why) => ui.muted(why.clone()),
                }
            });
            ui.time_axis(Clock::Beats { per_beat: BEAT, beats_per_bar: 4 }, FourTrack::view, FourTrack::playhead, |axis| {
                for track in self.tracks.iter() {
                    let id = track.id;
                    let name = track.name.clone();
                    let name = if track.muted { t!("{name}, muted") } else { t!("{name}") };
                    axis.track(name.as_str(), 2, |lane| {
                        lane.on_drag(move |drag| Cmd::Dragged(id, drag));
                        let dragging = lane.drag().filter(|d| d.moved).cloned();
                        let shown: Vec<&Clip> = self.clips.iter().filter(|c| c.track == id && lane.shows(c.at, c.end())).collect();
                        for clip in shown {
                            let chosen = self.chosen.contains(clip.id);
                            lane.span(clip.at, clip.end()).ink(if chosen { Ink::Selection } else { Ink::Field }).fill();
                            lane.span(clip.at, clip.end()).ink(if chosen { Ink::Link } else { Ink::Rule }).stroke(Weight::Line);
                            if let Some(sound) = self.sounds.get().and_then(|s| s.get(clip.sound)) {
                                let ink = if track.muted { Ink::Faint } else { Ink::Muted };
                                self.wave(lane, clip, sound, ink);
                            }
                            lane.label(clip.at, self.named(clip.sound).as_str()).ink(Ink::Ink).backed();
                        }
                        if let Some(drag) = dragging
                            && let Some(clip) = self.clip_at(id, drag.from.0 as i64 + self.view.start)
                        {
                            let at = self.moved_to(clip, &drag);
                            lane.span(at, at + clip.len).ink(Ink::Link).stroke(Weight::Bold);
                        }
                        if let Rolling::Recording(from, .., on) = self.rolling
                            && on == id
                            && deck.at > from
                        {
                            lane.span(from, deck.at).ink(Ink::Danger).stroke(Weight::Bold);
                        }
                        if deck.playing && self.rolling != Rolling::Stopped {
                            let x = lane.x(deck.at);
                            lane.line((x, 0.0), (x, 1.0)).ink(Ink::Link).stroke(Weight::Bold);
                        }
                    });
                }
            });
            ui.split(|ui| {
                ui.pane(t!("Tracks"), Fill, |ui| {
                    ui.list(&self.tracks, FourTrack::track_chosen, |ui, t| {
                        ui.text(&t.name);
                        ui.link(if t.muted { Cmd::Unmute(t.id) } else { Cmd::Mute(t.id) });
                    });
                    ui.row(|ui| {
                        ui.button(Cmd::AddDrums);
                        ui.button(Cmd::AddBass);
                        ui.button(Cmd::AddKeys);
                        ui.button(Cmd::AddLead);
                    });
                });
                ui.pane(t!("Clips"), Fill, |ui| {
                    if self.clips.is_empty() {
                        ui.muted(t!("Choose a track and add a loop, or record on it"));
                    }
                    ui.list(&self.clips, FourTrack::chosen, |ui, c| {
                        ui.text(self.described(c));
                        ui.link(Cmd::Remove(c.id));
                    });
                    ui.row(|ui| {
                        ui.button(Cmd::Repeat);
                        ui.button(Cmd::Split);
                    });
                });
            });
        });
    }

    fn states() -> States<Self> {
        let song = FourTrack::song();
        States::new()
            .add("A song of four loops", song.clone_song(|_| {}))
            .add(
                "Two clips chosen and zoomed in",
                song.clone_song(|s| {
                    s.view = TimeView::new(BAR * 2 - BEAT, 1_200);
                    for k in [5, 9] {
                        let _ = s.chosen.choose(&Choose::Toggle(k));
                    }
                }),
            )
            .add(
                "Playing, with the bass muted",
                song.clone_song(|s| {
                    let _ = s.tracks.change(Key::fixed(2), |t| t.muted = true);
                    s.rolling = Rolling::Playing(0, 1);
                    s.deck = Live::new(Deck { at: BAR * 3 + BEAT, cue: 1, level: 0.6, playing: true, device: Ok("Speakers".into()) });
                }),
            )
            .add(
                "Recording on the lead",
                song.clone_song(|s| {
                    let _ = s.track_chosen.choose(&Choose::Only(4));
                    s.playhead = BAR * 4;
                    s.rolling = Rolling::Recording(BAR * 4, 2, 3, Key::fixed(4));
                    s.deck = Live::new(Deck { at: BAR * 5, cue: 2, level: 0.3, playing: true, device: Ok("Speakers".into()) });
                }),
            )
            .add("An empty song", FourTrack::default())
    }
}

impl Default for FourTrack {
    fn default() -> Self {
        FourTrack {
            tracks: ["Drums", "Bass", "Keys", "Lead"].iter().enumerate().map(|(k, name)| Track { id: Key::fixed(k as u64 + 1), name: name.to_string(), muted: false }).collect(),
            clips: List::default(),
            view: TimeView::new(0, BEAT / 4),
            playhead: 0,
            rolling: Rolling::Stopped,
            snap: Snap::default(),
            chosen: Selection::default(),
            track_chosen: Selection::default(),
            sounds: Derived::default(),
            mix: Derived::default(),
            deck: Live::default(),
        }
    }
}

impl FourTrack {
    /// The sample song: the band's four loops on their own tracks, coming in one after another.
    fn song() -> FourTrack {
        let mut s = FourTrack::default();
        let mut id = 1;
        for (n, part) in [Part::Drums, Part::Bass, Part::Keys, Part::Lead].into_iter().enumerate() {
            for k in n..4 {
                let _ = s.clips.push(Clip { id: Key::fixed(id), track: Key::fixed(n as u64 + 1), sound: Source::Band(part), at: k as i64 * LOOP, from: 0, len: LOOP });
                id += 1;
            }
        }
        let _ = s.track_chosen.choose(&Choose::Only(1));
        s
    }

    fn clone_song(&self, change: impl FnOnce(&mut FourTrack)) -> FourTrack {
        let mut s = FourTrack::default();
        let _ = s.clips.replace(self.clips.iter().map(|c| Clip { id: c.id, track: c.track, sound: c.sound, at: c.at, from: c.from, len: c.len }));
        s.track_chosen = self.track_chosen.clone();
        change(&mut s);
        s
    }

    fn mixed(clips: &List<Clip>, tracks: &List<Track>, sounds: &Derived<Sounds>, rolling: &Rolling) -> Mix {
        let heard = |track: Key<Track>| tracks.find(track).is_some_and(|t| !t.muted);
        let voices = sounds
            .get()
            .map(|sounds| {
                clips
                    .iter()
                    .filter(|c| heard(c.track))
                    .filter_map(|c| sounds.get(c.sound).map(|sound| Voice { sound: sound.clone(), at: c.at, from: c.from, len: c.len.min(sound.len() - c.from), gain: 0.8 }))
                    .collect()
            })
            .unwrap_or_default();
        let transport = match *rolling {
            Rolling::Stopped => Transport::Stopped,
            Rolling::Playing(from, cue) => Transport::Playing { from, cue },
            Rolling::Recording(from, cue, take, _) => Transport::Recording { from, cue, take },
        };
        Mix { voices, transport, folder: folder() }
    }

    fn add(&mut self, part: Part, cx: &mut Cx<Cmd>) -> Outcome {
        let Some(track) = self.track_chosen.focused() else { return Outcome::Nothing };
        let at = self.clips.iter().filter(|c| c.track == track && c.end() > self.playhead && c.at <= self.playhead).map(Clip::end).max().unwrap_or(self.playhead);
        let id = cx.key();
        let _ = self.clips.push(Clip { id, track, sound: Source::Band(part), at: self.snap.to(at), from: 0, len: LOOP });
        self.chosen.choose(&Choose::Only(id.raw()))
    }

    fn zoom(&mut self, closer: bool) -> Outcome {
        self.view = self.view.zoomed(closer, 0.0);
        Outcome::Changed
    }

    fn clip_at(&self, track: Key<Track>, t: i64) -> Option<&Clip> {
        self.clips.iter().rev().find(|c| c.track == track && c.at <= t && t < c.end())
    }

    fn moved_to(&self, clip: &Clip, drag: &Drag) -> i64 {
        self.snap.to(clip.at + (drag.to.0 - drag.from.0) as i64).max(0)
    }

    fn dragged(&mut self, track: Key<Track>, drag: Drag) -> Outcome {
        let t = drag.from.0 as i64 + self.view.start;
        let Some(clip) = self.clip_at(track, t) else {
            return self.chosen.choose(&Choose::Clear);
        };
        let id = clip.id;
        if !drag.moved {
            let choice = if drag.shift { Choose::Toggle(id.raw()) } else { Choose::Only(id.raw()) };
            return self.chosen.choose(&choice);
        }
        let at = self.moved_to(clip, &drag);
        let lanes = drag.to.1.floor() as i64;
        let from = self.tracks.position(track).unwrap_or(0) as i64;
        let onto = self.tracks.get((from + lanes).clamp(0, self.tracks.len() as i64 - 1) as usize).map_or(track, |t| t.id);
        if at == clip.at && onto == track {
            return Outcome::Nothing;
        }
        let _ = self.clips.change(id, |c| (c.at, c.track) = (at, onto));
        self.chosen.choose(&Choose::Only(id.raw()))
    }

    fn named(&self, source: Source) -> Message {
        match source {
            Source::Band(part) => part.name(),
            Source::Take(_) => t!("Take"),
        }
    }

    fn described(&self, c: &Clip) -> Message {
        let (name, track) = (self.named(c.sound), self.tracks.find(c.track).map_or(String::new(), |t| t.name.clone()));
        let (from, to) = (clock(c.at), clock(c.end()));
        t!("{name} on {track}, {from} to {to}")
    }

    /// Draws a clip's waveform as one shape, four steps to a column, only where it shows.
    fn wave<C, S>(&self, lane: &mut graticule::kernel::ui::Lane<'_, '_, '_, C, S>, clip: &Clip, sound: &Sound, ink: Ink) {
        let step = (self.view.per_col / 4).max(1);
        let first = clip.at.max(self.view.start);
        let mut t = first - (first - clip.at) % step;
        let mut edge = Vec::new();
        while t < clip.end() && lane.shows(t, t + step) {
            let from = clip.from + (t - clip.at);
            let (lo, hi) = sound.peaks(from, from + step);
            edge.push((lane.x(t), 0.5 - hi.max(0.01) * 0.4, 0.5 - lo.min(-0.01) * 0.4));
            edge.push((lane.x((t + step).min(clip.end())), 0.5 - hi.max(0.01) * 0.4, 0.5 - lo.min(-0.01) * 0.4));
            t += step;
        }
        let Some(&(x, top, _)) = edge.first() else { return };
        lane.path(|pen| {
            pen.move_to((x, top));
            for &(x, top, _) in &edge[1..] {
                pen.line_to((x, top));
            }
            for &(x, _, bottom) in edge.iter().rev() {
                pen.line_to((x, bottom));
            }
        })
        .ink(ink);
    }
}

/// A time as bars and beats, counting from 1.
fn clock(t: i64) -> String {
    let beats = t.max(0) / BEAT;
    format!("{}.{}", beats / 4 + 1, beats % 4 + 1)
}

/// The whole song mixed down, as a WAV file.
fn bounce(s: &FourTrack) -> Vec<u8> {
    s.mix.get().map_or_else(Vec::new, |mix| mix.render(0, mix.end()).wav())
}

fn main() {
    graticule::run::<FourTrack>(t!("Four-track"));
}

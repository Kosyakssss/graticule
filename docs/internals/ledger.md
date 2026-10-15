# Ledger: the store, the command log and the CRDT

Built in S5. How Ledger keeps facts on disk, why the CRDT is Loro, and why saving doesn't go through it yet.

## The store

`ledger::Store` owns one folder per app (`Graticule/<program name>` in the system's app data folder, from `kernel::runner::folder`). It holds four files:

- `replica`: this device's id, made on the first launch and kept, so keys made on this device never collide with another's. The top bit is always set, so it never equals the replica `Key::fixed` uses.
- `facts`: a snapshot, the `Value` of the whole facts plus the format version, the key counter and how far into the log it reaches.
- `log`: every committed entry since the first launch, appended as `[length u32][FNV-1a checksum u32][bytes]`, each record holding the entry (what the user did), its operations and the key counter after it.
- `place`: where the user was (focus, caret, scroll anchors), written when the app closes.

Opening reads the snapshot, then applies the log records after its offset. A record cut short or with a bad checksum ends the log there: the file is truncated to the last whole record and the message line says the last change was left out. A snapshot that can't be read is renamed aside, never deleted, and the app starts fresh with a message saying where it went.

Every write happens on a thread named `ledger`. The main thread sends the entry and its operations over a channel, so its cost is the size of the change. The thread applies the operations to its own copy of the facts, so writing a snapshot never needs the main thread. It writes a snapshot every 2000 entries and on closing; snapshots and `place` are written to a new file and renamed over the old one, so a crash leaves either the old file or the new one. Write errors come back through `Store::trouble` and show as failures in the message line.

The log keeps growing: it is the command log as well as the crash record, and the `ledger` command (item 21) reads it to show history and replay it. Trimming it is left for when its size matters.

## The face

`--ledger` on any app's command line (`graticule::run`, in `crates/graticule/src/face.rs`) reads its store without writing to it: `ledger::read` loads the snapshot and applies the whole log records after it, the same loop `Store::open` uses, but sets nothing aside and truncates nothing. History replays the log from the app's fresh facts one entry at a time, so each command is labelled from the facts as they were just before it ran, which is what its label read when it was chosen; entries that are only the rest of a long change (`turn`, `slice`) are not listed. Replay prints the facts after N entries as an indented tree; verify replays everything and compares with what the store holds, path by path; compare reads two stores and lists where their facts differ. A replay runs undo whole rather than in slices, as replays always do.

## Replays

An entry is a command (its commands as `Value`s, and the replica, counter, time and seed `Cx` handed out), or "undo", or "redo". `Runner::replay` runs entries again through the same transaction path with the same clock, so keys, times and random numbers come out the same and the facts do too (the law `replaying_the_command_log_gives_the_same_facts`). Focus repair after a removal is its own entry, so a replay switches repair off rather than doing it twice.

## Moves and rows that are gone

A change to a selection alone is not an undo step, but history keeps it as a move, so undoing the next step also puts the selection back where it was. A session list can lose rows without an undo step, as when Specimen closes a page and its lines go. A move that names those rows would bring back, on undo, a selection of rows that no longer exist, since the rows themselves don't come back. So when a change that isn't a step deletes rows, history forgets every move that names any of their keys, pending or kept with a step. It compares raw keys, so a key of another list with the same value is forgotten too; that costs only a selection undo would have restored. The walk is over the whole history, and happens only on such a deletion.

## Why Loro

Measured in S5 with a throwaway program (not in the repository), release build on the development machine, 100,000 records of three fields, a commit after every change:

| | Loro 1.16.2 | Automerge 0.12 |
| --- | --- | --- |
| insert a record | 3.6 µs | 70 µs |
| toggle a field | 1.6 µs | 41 µs |
| type a character | 1.3 µs | 17 µs |
| remove a record | 1.4 µs | 19 µs |
| memory after the edits | 140 MB | 70 MB |
| snapshot | 8.7 MB (4.6 MB without history) | 1.9 MB |
| save a snapshot | 0.32 s | 0.11 s |
| load a snapshot | 31 ms | 758 ms |
| merge 1000 + 1000 new records | 204 ms | 60 ms |
| one change as an update | 90 bytes | 113 bytes |

Loro is ten to twenty times faster per change and twenty-five times faster to load; Automerge is smaller in memory and on disk and merges faster. Changes happen all the time and loading happens at every launch, while large merges are rare, so Loro. Both would run on Ledger's thread, never on the main thread.

## Why saving doesn't go through the CRDT yet

The spec first had the CRDT save the facts. Ledger's own snapshot is a plain `Value`: it loads in the time it takes to decode, it is readable by a newer version through declared migrations (field names, not CRDT internals), and it needs no CRDT at all on a single device. The CRDT document is needed once there is a second device, and it can be built then from the snapshot. So for now Ledger saves its own format, and the question of whether to switch is spec §15, question 3, for the owner.

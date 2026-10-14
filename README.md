# Graticule

A framework for native desktop apps in Rust, built on a grid. An app describes its facts, the commands that change them and how they look; Graticule does the rest, faster than any display can show and silent when idle.

It is being built now, item by item, from the build plan at the end of the spec. Start with:

- [`docs/spec.md`](docs/spec.md): what we're building and the build plan.
- [`docs/api.md`](docs/api.md): the app code we want to be able to write.
- [`docs/guide/`](docs/guide/README.md): all of it in plain words.
- [`AGENTS.md`](AGENTS.md): how to work on it.

```
cargo run -p todo                        # the to-do app
cargo run -p todo -- --specimen          # Specimen over its named states
cargo run --release -p todo -- --assay   # every law and budget, as a report
cargo run -p specimen                    # Specimen on its own
cargo run -p scope                       # attach to a running app and watch its frames
cargo run -p bindery -- new apps/hello   # start an app
cargo run -p bindery -- dev apps/hello   # rebuild and reload it on every saved change
```

Licensed under the [Mozilla Public License 2.0](LICENSE).

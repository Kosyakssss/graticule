# Text, fonts and character graphics

Moved in from Feedreader's Graticule in S2. These are the traps that are hard to see from the source.

## Fonts are the bundled ones, the system's only as a fallback

IBM Plex is compiled into the binary (`crates/kernel/assets/fonts`), and the text system starts with only Plex Mono and Plex Sans. On an M1, registering them takes about 0.3 ms and a first line of text 0.03 ms; letting fontique scan the system's fonts cost about 35 ms at start and let the system choose fonts, so two machines could paint differently.

Plex's other families (Sans Arabic, Devanagari, Hebrew, Thai and Plex Math) are compiled in too and registered the first time text needs them (`fonts::EXTRA`), each put first among the fallbacks for its scripts. Plex Sans JP, KR, SC and TC are too large for the repository: `assets/fonts/fetched.txt` pins each file's URL and SHA-256, Bindery downloads them into the shared data folder (`Graticule/Fonts`), and the kernel reads them from there the first time text needs them. Han characters are shared by all four, so the choice follows the text (`fonts::Han`): kana make it Japanese, hangul Korean, anything else Simplified Chinese.

Only a character no Plex family covers (emoji, or Han while the fetched fonts are absent) makes the text system load the system's fonts, once, on a thread of its own; the window repaints when they arrive. Offscreen renders wait for that load and paint again, so pictures are the same every time. When the fonts arrive the glyph atlas is cleared, so painted text kept from the last frame is shaped again, and the window lays out again, because text measured before the load (a Chinese label measured as boxes) has a different width after it.

## Measuring counts trailing spaces

`Text::measure` uses parley's `full_width`, not `width`. `width` leaves out trailing whitespace, so anything placed after a measured prefix that ends in a space (the caret, the rest of a field's text, a selection) lands one space too far left per space. An empty string measures zero: parley gives an empty line the width of a caret.

## The cell is whole points

The cell's width is the advance of Plex Mono at 13 pt rounded to a whole point, and the font size is then derived from the cell; the height is 1.6 lines, rounded to a whole point (`metrics.rs`). At integer scales a cell is whole device pixels, so a window keeps its column count when it moves between a 1x and a 2x display. At fractional scales (Windows at 125 % or 150 %, open question 5 for Linux) the device size is rounded to whole pixels, so the column count can differ by one.

## Character graphics are drawn, not taken from the font

Box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and braille (U+2800–U+28FF) are drawn by `chars.rs` as rectangles snapped to device pixels, so lines meet exactly at cell edges whatever the font, size or scale. Each box character is four arms (up, right, down, left) of none, light, heavy or double weight. An arm's inner end depends on the arms across it, so joins have no gaps: a single line covers the line it meets, and of a double line's two strokes, the one on the side of a crossing arm stops at that arm's near stroke while the outer one runs to its far stroke. Arcs and diagonals are paths rasterized once per cell size into the glyph atlas. Mono text that contains any of these characters, or any character wider than one cell, is laid out cell by cell (`Painter::cells`), unless it also holds a script that must be shaped as a whole (Arabic, Hebrew, the Indic scripts, Thai, combining marks: `text::shaped`). Drawn a cell at a time, Arabic loses its joins and reads left to right; shaped text is measured instead, and takes as many cells as its width needs.

## Painting text allocates nothing in steady state

`Text::draw_spans` keeps its glyph runs, variation coordinates and rules in scratch vectors that live as long as `Text`, and the character-graphics pen is kept there too, so a frame that draws text it has drawn before allocates nothing (the "no allocation per frame" law in `crates/kernel/src/laws.rs`). Layouts are cached by a hash of their spans, size, line and wrap width; a new text allocates its layout once.

# The shell

The shell (`crates/kernel/src/shell.rs`) is the part of the kernel that meets the operating system: the event loop, the window, input, the GPU surface. It runs a `Face`, which paints frames and answers input; the app runner (S3) is one face, and Specimen's offscreen pages use the same painter without a window.

## Frames come only from causes

The event loop waits (`ControlFlow::Wait`); there is one timer, and only while a date written as how long ago is on screen (below). Each input goes to the face, which says whether anything changed; only then does the shell ask the window for a redraw, and only if one isn't already owed (`Pace`), so a burst of events between two frames makes one frame. A resize repaints at once, inside the resize event, so the window never shows a stretched frame. The "idle is zero" law in `crates/kernel/src/laws.rs` checks the pacing over generated sequences of events.

## The window is hidden until its first frame

The window is created hidden, the first frame is drawn into it, and only then is it shown, so the first thing on screen is the finished UI, never an empty window. `GRATICULE_SCOPE=1` prints the launch stages once.

On the Windows development machine, a debug build spends about 1.1 s of its launch creating the GPU device (wgpu enumerating DirectX 12 and Vulkan adapters); everything else is under 20 ms. Feedreader's measurements on an M1 put the GPU at 4 ms and the whole launch at 155 ms, most of it AppKit. Budgets are judged on the M1 in release builds.

## Where it was built

The first slice is written and verified on Windows 11, because that is the machine the work happens on; the spec says macOS first. winit and wgpu make the shell the same code on both, but nothing macOS-specific (the menu bar, the context menu, Quick Look) can be verified here, and those items wait for a Mac.

## Dates that move

A date written as how long ago ("5 min ago") reads differently as time passes, with no input at all. The view writes dates when the tree is built, and while it does, the tree keeps the first moment any of them would read differently (`Tree::changes`; `Calendar::write` returns it beside the text). After each batch of events the shell asks the face for that moment (`Face::changes`) and waits until then (`ControlFlow::WaitUntil`) instead of forever. When it arrives the face gets `Input::Clock`, builds the view again with the clock read afresh, and Scope counts the frame under the cause "a date written as how long ago changing". So the window wakes once a minute while a date under an hour old shows, once an hour under a day, and once at midnight otherwise; with every date written as the day and time, which is the default, it never wakes. This is the one timer in the loop, and it is an open question in the spec (§15) whether to keep it.

Dates are written against a calendar: the user's setting, the moment it is now and the time zone (`jiff`, which reads the system's zone). A runner starts with a fixed calendar, noon on 1 June 2026 in UTC, and becomes live only when the shell hands it the looks. Proofs, Assay and the laws never do, so a date renders the same wherever and whenever they run.

## Menus

On macOS the shell puts an app menu (About, Services, Hide, Quit) and a Window menu in the menu bar at launch (`menu::install`), so the system's shortcuts and window tiling work. A right click asks the face for a context menu (`Face::menu`): the runner chooses the row under the pointer unless it is already chosen, builds the tree, and turns the bar's controls into entries, the row's commands on a row and the commands without a subject elsewhere, with Copy first while text is chosen. Taking entries from the bar means a menu shows exactly the labels, keys and availability the bar does; an entry that can't run says why in its label, since a native menu has no status line. The shell paints the newly chosen row before the menu opens, because AppKit blocks inside the menu call. A choice, from the bar or a menu, comes back through muda's event handler as a `Signal::Menu`, after the menu has closed, and runs the entry's control as a click would (`Face::choose`). Menus open at the mouse: muda reads the mouse location itself when given no position, which also avoids its upside-down conversion of points in winit's flipped view.

Only macOS draws these menus natively, through the one approved `unsafe` (`docs/internals/unsafe.md`); on Windows and Linux a right click chooses the row and nothing opens until the drawn menu arrives with the anchored overlays (item 14). None of this has run on a Mac yet: it compiles for `aarch64-apple-darwin`, and its first pass on a real Mac is owed.

## Input methods

While a field has focus the window accepts input methods and places the candidate window at the caret (`set_ime_cursor_area`), measured the way the field draws its text. What the input method is composing is the runner's, not the facts': it is drawn in the field at the caret, underlined, until it is committed or dropped, and keys go to the input method rather than to commands while it composes. Monospace text containing wide characters is drawn cell by cell, two cells for a CJK character, so carets, the composition and selections measure it the same way (`paint::advance`) rather than by its shaped width.

## Choosing text

Text and values on a page can be chosen with the pointer, across components: a drag from one text to another chooses everything between them in reading order, two clicks choose a word and three a whole text, and a value is always chosen whole. The ends are kept by node id and byte, so the choice stays on the same text when the tree is built again and is dropped when that text is gone. "Copy the chosen text" (the built-in `copy`, the platform's copy key when no field has focus) copies it, pieces on one row joined by a space and rows by a new line, and Escape lets it go. Controls and list rows act when clicked, so their text isn't chosen this way.

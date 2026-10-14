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

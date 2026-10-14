# Proofs and Specimen's Changes place

A proof is what a named state looks like, kept so that a later change to it can be found, said in words and approved. Open question 2 asks whether proofs are images, text descriptions of the tree, or both. For now they are text, and images are not kept: text is small, reads in a diff, and is what the words come from. Before-and-after pictures, blink and a heatmap need a component that shows a picture, which doesn't exist yet. If the owner decides otherwise, `kernel::proof` and `specimen::changes` are the only places that change.

## What a proof holds

`kernel::proof::describe` walks the laid-out tree and writes one line per component that has room on screen: its depth, role, label, area in cells (column, row, width, height) and state (ticked, chosen, unavailable and why, muted, the items a list holds). The fields are separated by tabs, with tabs, newlines and backslashes in labels escaped. Ids are left out because they come from lines of code, so moving code would read as a change. Keys are left out because they are written differently on each platform.

Each named state is proved at the normal size, 1920 by 1280 at scale 2, and at the smallest window it allows. The tree doesn't depend on the appearance, so light is enough.

## Where proofs live

`proofs/<app>/<platform>/<state>, <size>.txt`, in the workspace Specimen was started in. The workspace is found by walking up from the current folder to the one with `Cargo.lock`; outside a workspace they go in the app's data folder. Each platform has its own proofs because the bar's layout follows how keys are written ("Ctrl+Z", "⌘Z"). Specimen's own pages show Chinese and emoji, which are measured with the system's fonts when the fetched Plex fonts are missing, so their proofs can differ on a machine that lacks them. The to-do app's proofs don't have this problem.

## How changes are said

`kernel::proof::changes` matches the two proofs' lines by depth, role and label (the longest common sequence) and then says:

- for a matched component: how far it moved, how much it grew or shrank, and which states it gained or lost;
- for a pair it couldn't match: "now says …" when the old and new lines have the same role and depth, otherwise "is gone" or "appeared";
- moves after the others, with a run of components that moved the same way said once ("and 4 more after it").

Containers (groups, columns, pages, list rows, lists, the bar, the status and message lines) aren't reported for moving or resizing: when they change, their contents do too, and the contents are reported. A change to the window's smallest size is said once. The same sentence from several places is said once, with the count.

## The Changes place

The first time Changes opens, Specimen draws every sheet on a thread of its own with an offscreen renderer and compares each with its proof; the place shows that it is looking until the thread wakes the window. The sheets then become the facts of a small app (`specimen::Changes`), so approving is a command like any other. It is in the bar, the palette and on each row, and works on a selection of several. Approving writes the proof through `Effect::Write`, off the main thread. Rejecting needs nothing: a sheet that is not approved stays on the list until the code is fixed.

`--changes` does the same without a window: it prints the words and fails when anything differs or was never approved. `--approve` approves every sheet as it renders now.

## The page for review

`--render <folder>` writes `index.html` beside the pictures: every named state with its four pictures and the words from the survey, so a review needs only a browser. Reference images stay outside the repository; `--references <folder>` pins any picture whose name is a state's name, or starts with it and a comma ("Empty, the mockup.png"), beside that state, linked by its path rather than copied.

# The components

One page per component lives beside this file, and Specimen shows each page next to the live component. This page is the inventory: every component of HeroUI and Blueprint, what it becomes in Graticule, and the wave it is built in (build-plan item 15). The owner removes the ones Graticule doesn't keep afterwards, in Specimen.

The sources are HeroUI 3.2.6 (`@heroui/react`, built on React Aria) and Blueprint 6 (`@blueprintjs/core`, `datetime`, `select`, `table`), read from their repositories in October 2026. From both we take what a component is and how it behaves (states, keyboard, focus, what a screen reader says), never how it looks.

Several names on either side are the same component. They are merged into one row here, with both names given.

**Grounded** means the component is drawn on the grid in the page instead of floating (spec §4.4):

- a tooltip becomes the status line's hint;
- a toast becomes the message line;
- a dialog becomes an inline confirm row;
- a dropdown becomes a cycling value, or a choice that opens inline.

**Anchored** means it is one of the overlays the spec allows: opened by an explicit action, anchored to what opened it, on the grid with a hard shadow, and closed by Escape or by clicking away.

## Done

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Page | | | the scrolling page every place is drawn in |
| Row, Columns | | | layout in cells |
| Text | Typography | Text | plain, muted, mono |
| Button | Button | Button | |
| Link | Link | Link | |
| Check | Checkbox | Checkbox | |
| List | ListBox | | selection, ranges, keyboard |
| Empty | EmptyState | NonIdealState | |
| Swatch | ColorSwatch | | |
| Picture | Avatar (image part) | Icon | Engraver's pictures and the built-in icons |
| Canvas | | | shapes and words in a space of the app's own, drawn off the main thread; shapes can be pressed (spec §6.3, item 16) |
| Image | Image | | PNG or JPEG, kept as a resource, keeping its shape in a box of cells (item 16) |
| Paint hook | | | the app's own GPU commands under a canvas (spec §6.4, item 16) |
| Values | Badge | | count, date, version, state, product code, code, mode, error (§5.2); a state is the badge |
| Action bar | Toolbar | | the registry along the bottom |
| Status line | Tooltip | Tooltip | grounded: a hint, never a floating tip |
| Message line | Toast | Toast, Toaster | grounded |
| Palette | | Omnibar | |
| Toggle button | ToggleButton | Button (active) | a button that stays on |
| Button group | ButtonGroup, ToggleButtonGroup | ButtonGroup, SegmentedControl | joined buttons and toggles; segments choose one option of a fact |
| Close button | CloseButton | | an icon button with the close icon; any picture works the same way |
| Radio group | RadioGroup, Radio | RadioGroup, Radio | |
| Check group | CheckboxGroup | | a fieldset of check boxes |
| Switch | Switch, SwitchGroup | Switch | an on/off fact, applied at once |
| Label, description, error | Label, Description, FieldError, ErrorMessage | Label, FormGroup helper text | the parts of a labelled control |
| Fieldset | Fieldset, Form | FormGroup, ControlGroup | controls grouped under a heading |
| Key label | Kbd | KeyComboTag | drawn from the keymap, never typed by hand |
| Separator | Separator | Divider | a rule on a cell edge |
| Spinner | Spinner | Spinner | the status line says what is running, in words; nothing spins, since nothing moves by itself (§4.5) |
| Progress | ProgressBar, ProgressCircle | ProgressBar | work the app counts; a job's own progress is in the status line |
| Meter | Meter | | a value within a range |
| Placeholder | Skeleton | Skeleton | rows reserved while data arrives late |
| Tag | Chip, Tag, TagGroup | Tag | removable tags are wave 2 |
| Avatar | Avatar, AvatarGroup | | a person's initials; their picture cropped into it is not done |
| Callout | Alert | Callout | a boxed message in the page |
| Card | Card, Surface | Card, CardList, Section, ControlCard | a boxed group with a heading |
| Heading | Header | EntityTitle | strong text announced as a heading; with its icon and typed values, a row of them |
| Tabs | Tabs | Tabs | the tabs are an enum fact; only the chosen page is described |
| Navigation | | Navbar | places along the top, the current one marked |
| Pagination | Pagination | | only where paging is the data's own shape |
| Disclosure | Disclosure, DisclosureGroup, Accordion | Collapse, Section (collapsible) | sections that open in place |
| Breadcrumbs | Breadcrumbs | Breadcrumbs | long paths shortened in the middle |
| Panel stack | | PanelStack | drill in and back, in place: a fact for the depth and breadcrumbs, not a component of its own |
| Overflow | | OverflowList | what doesn't fit goes into a "more" choice |
| Scroll edges | ScrollShadow | | every list draws a rule at an edge with more beyond |
| Text field | TextField, Input, InputGroup | InputGroup | one line; pieces beside it are other components in a row with it |
| Text area | TextArea | TextArea | several lines, wrapped, growing with the text |
| Search field | SearchField | InputGroup (search) | the app filters by its text (§7.4); Escape empties it |
| Editable text | | EditableText | text that becomes a field when you act on it |
| Number field | NumberField | NumericInput | whole numbers, typed as sums and stepped with the keys; units are not done |
| Slider | Slider | Slider | one whole number in a range |
| Code field | InputOTP | | one character per box |
| Form | Form | FormGroup | settings drawn from a struct of facts, one control per field by its type |
| Choice | Select (native) | HTMLSelect | grounded: a button that cycles through the options |
| File field | | FileInput | a path typed in, or chosen with the system's file panel |
| Tag field | TagGroup (removable) | TagInput | a list of words, each removable, and a field to add another |
| Range slider | Slider (range) | RangeSlider | two handles on one track, each its own tab stop; more handles are not done |
| Select | Select, ListBox | Select | grounded: a choice that opens inline, its options under it; sections are not done |
| Combo box | ComboBox, Autocomplete | Suggest | anchored: the completions that contain what is typed, under the field while it has the focus |
| Multi-select | Select (multiple) | MultiSelect | grounded: the same select, ticking options on and off in a list |
| Menu | Dropdown, Menu, MenuItem | Menu, MenuItem | anchored: the controls it holds as entries, with their keys and reasons, drawn like the context menu; sections are not done |
| Context menu | | ContextMenu | from the registry: an anchored overlay on Windows and Linux; the native menu on macOS waits for the Mac |
| Popover | Popover | Popover | anchored: a few controls under a button, built only while open |
| Confirm row | AlertDialog, Modal | Alert, Dialog | grounded: a button that asks in its own place before running its command |
| Side panel | Drawer | Drawer | grounded: a fact and a column, not a component of its own |
| Help page | | Hotkeys dialog | the registry as a tree with its keys (§8.3, item 12) |
| Calendar | Calendar | DatePicker (the month grid) | a month a week to a row, one tab stop; ranges and a year picker are not done |
| Date field | DateField, DateInputGroup | DateInput | a day typed as a date or in words (`next tue`, `jun 3`); a day and time together and ranges are not done |
| Date picker | DatePicker | DatePicker | anchored: a date field and a calendar in a popover; ranges and shortcuts are not done |
| Time field | TimeField | TimePicker | a time of day (`3pm`), stepped by the hour or minute the caret is in |
| Time zone | | TimezoneSelect | a combo box over the zones' names |
| Color field | ColorField, ColorInputGroup | | a swatch and the color typed as hex or OKLCH, shown as hex |
| Color area and slider | ColorArea, ColorSlider | | lightness and chroma as an area at the color's hue, each part as a slider over its colors |
| Color picker | ColorPicker | | anchored: a color field and an area and hue slider in a popover; swatch sets are not done |
| Table | Table | Table, HTMLTable | wave 6: a list of cells in named columns, sorted or filtered by an order worked out in the background, columns resized by dragging; grouping, the glow on change and resizing from the keyboard are not done |
| Tree | | Tree | wave 6: a list whose rows sit inside one another, its open branches a fact and its rows worked out in the background; Right and Left open, close and move between levels |
| Picker | | Omnibar, QueryList | wave 6: a search field over the items that match it, best first, worked out in the background, with the letters typed marked; Up, Down and Return choose |
| Typed filter | | | wave 6: `people>1000000 country:spain` against a record's own fields, offering them as they are typed; days and times are not done |
| Table from data | | | wave 6: a column for each of a record's fields, sorted by the field a column is named for; forms from data are `ui.form` |
| Time axis | | | wave 6: a ruler counted in numbers, seconds or bars and beats over tracks whose lanes are canvases in time; the playhead snaps to the marks; zoom and scroll by the wheel |
| Chart | | | wave 6: lines, bars and candles on a canvas with scales on whole cells, sized in rows; sparklines in table cells; readouts under the pointer are not done |
| Node graph | | | wave 6: boxes holding what the app gives, set out in layers by a layout worked out off the main thread, wires in tracks of their own; listed when narrow; keys along wires, dragging and drawing wires are not done |
| Split panes | | | wave 6: panes side by side with dividers dragged or moved by the arrows, the widths kept with the window, stacked when narrow; docking and layouts saved as files are not done |

## Wave 6: beyond both kits

These are the components of spec §6.1 that professional apps need and that neither kit has. They need the canvas (item 16) and jobs in full (item 13).

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Docking | | | panes that move between places; layouts saved as files |
| Find | | | find and replace in a document, with Galley (§7.4) |
| Diff view | | | |
| Code and text view | | | Galley (§7.3, §13.3) |

## Not components

These fall out of the architecture, or belong to the kernel's insides. Nothing is built for them.

| HeroUI | Blueprint | Why |
|---|---|---|
| | Overlay, Overlay2, Portal | the kernel places anchored overlays itself |
| | ResizeSensor | layout knows every box's size |
| | Hotkeys (provider) | the keymap (§3.4) |
| rac | | React Aria's plumbing |
| | html elements (H1, Pre, Code, …) | Text and its styles |
| | Icon packages | the built-in icons and Engraver |

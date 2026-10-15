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
| Avatar | Avatar, AvatarGroup | | a person's initials; their picture comes with raster images (item 16) |
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

## Wave 2: fields in full

These need the full field (spec §7.2, build-plan item 11): input methods, selection and multi-line.

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| File field | | FileInput | opens the system's file panel |
| Tag field | TagGroup (removable) | TagInput | |
| Range slider | Slider (range) | RangeSlider, MultiSlider | a range, or several handles; one value is done |
| Choice | Select (native) | HTMLSelect | grounded: a cycling value |

## Wave 3: choices that open

These need anchored overlays (build-plan item 14) or the grounded forms of overlays.

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Select | Select, ListBox sections | Select | grounded: a choice that opens inline |
| Combo box | ComboBox, Autocomplete | Suggest | anchored: completions under the field |
| Multi-select | Select (multiple) | MultiSelect | |
| Menu | Dropdown, Menu, MenuItem, MenuSection | Menu, MenuItem | built from the registry (§3.3) |
| Context menu | | ContextMenu | from the registry: the native menu on macOS, an anchored overlay elsewhere |
| Popover | Popover | Popover | anchored |
| Confirm row | AlertDialog, Modal | Alert, Dialog | grounded: an inline row that asks |
| Side panel | Drawer | Drawer | grounded: a pane that docks, not a sheet over the page |
| Help page | | Hotkeys dialog | the registry as a tree with its keys (§8.3, item 12) |

## Wave 4: time

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Calendar | Calendar, RangeCalendar, CalendarYearPicker | DatePicker, DateRangePicker (the month grid) | |
| Date field | DateField, DateInputGroup | DateInput, DateRangeInput | accepts natural input (`next tue 3pm`) |
| Date picker | DatePicker, DateRangePicker | DatePicker, DateRangePicker, shortcuts | anchored: field and calendar |
| Time field | TimeField | TimePicker | |
| Time zone | | TimezoneSelect | a select over the zones |

## Wave 5: color

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Color field | ColorField, ColorInputGroup | | OKLCH and hex |
| Color area and slider | ColorArea, ColorSlider | | drawn on a canvas inside the box |
| Color picker | ColorPicker, ColorSwatchPicker | | anchored |

## Wave 6: beyond both kits

These are the components of spec §6.1 that professional apps need and that neither kit has. They need the canvas (item 16) and jobs in full (item 13).

| Graticule | HeroUI | Blueprint | Notes |
|---|---|---|---|
| Virtual table | Table | Table, HTMLTable | millions of rows; sort, group, resize, filter in the background |
| Tree | | Tree | |
| Split panes and docking | | | layouts saved as files |
| Picker and find | | Omnibar, QueryList | one picker behind the palette; find and replace (§7.4) |
| Typed filter field | | | `price>100 sector:tech`, from the data types |
| Forms and tables from data | | | derived from the app's types |
| Time axis | | | rulers, playhead, zoom, snapping, tracks as rows |
| Charts | | | lines, bars, candles, sparklines in cells |
| Node graph | | | with automatic layout |
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

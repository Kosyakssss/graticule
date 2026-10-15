use graticule_macros::{Choice, Commands, Facts, Form};
use kernel::ink::{Appearance, contrast};
use kernel::layout::{GAP, MARGIN};
use kernel::metrics::{FONT_POINTS, LINE};
use kernel::{App, Available, Channel, Clock, Cx, Day, Derived, Failure, Image, Ink, Message, Oklch, Order, PaintHook, Path, Plot, Sort, States, Time, TimeView, Ui};
use ledger::{Edit, Key, List, Outcome, Selection, Text};

use crate::style::{self, Mark, Mix, Nudge, Reach, Reaches, Side, Weight};

/// Specimen's places. Every one is in the bar wherever you are.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Components,
    Screens,
    Style,
    Changes,
}

impl Place {
    pub const ALL: [Place; 4] = [Place::Components, Place::Screens, Place::Style, Place::Changes];

    pub fn name(self) -> &'static str {
        match self {
            Place::Components => "components",
            Place::Screens => "screens",
            Place::Style => "style",
            Place::Changes => "changes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Place::Components => "Components",
            Place::Screens => "Screens",
            Place::Style => "Style",
            Place::Changes => "Changes",
        }
    }
}

/// How a collection is laid out, the option the live examples of choices choose between.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    List,
    Grid,
    Board,
}

/// The pages of the live example of tabs.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    General,
    Appearance,
    Keys,
}

/// The places of the live example of navigation: more than fit in a narrow window.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq)]
pub enum Folder {
    #[default]
    Inbox,
    Drafts,
    Sent,
    Archive,
    Spam,
    Trash,
}

/// The paper of the live example of a form.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq)]
pub enum Paper {
    #[default]
    A4,
    Letter,
    Legal,
}

/// The live example of a form: settings for printing, one control for each by its type.
#[derive(Facts, Form)]
pub struct Printing {
    #[label("Copies")]
    copies: u8,
    #[label("Paper")]
    paper: Paper,
    #[label("Print on both sides")]
    both_sides: bool,
    #[label("Header")]
    header: Text,
}

impl Default for Printing {
    fn default() -> Self {
        Printing { copies: 1, paper: Paper::A4, both_sides: false, header: Text::default() }
    }
}

const LEVELS: [&str; 3] = ["Settings", "Account", "Password"];

const TRAIL: [&str; 6] = ["Home", "Projects", "Graticule", "docs", "components", "Breadcrumbs"];

/// Specimen's own facts: where it is, and the live examples on its pages, which are as real as any app's.
#[derive(Facts)]
pub struct Catalog {
    #[session]
    pub place: Place,
    #[session]
    sections: List<Section>,
    #[session]
    chosen: Selection<Section>,
    #[session]
    page: Option<Component>,
    #[session]
    blocks: List<Block>,
    #[session]
    reading: Selection<Block>,
    #[session]
    settings: List<Setting>,
    #[session]
    looking: Selection<Setting>,
    #[session]
    editing: Side,
    inks: List<Mix>,
    lines: Weight,
    shadow: u32,
    check: Mark,
    #[session]
    reach: List<Reach>,
    #[session]
    screens: u32,
    #[session]
    samples: List<Sample>,
    #[session]
    draft: Text,
    #[session]
    ticked: bool,
    #[session]
    pressed: u32,
    #[session]
    bold: bool,
    #[session]
    wrapping: bool,
    #[session]
    shape: Shape,
    #[session]
    pane: Pane,
    #[session]
    folder: Folder,
    #[session]
    open: bool,
    #[session]
    leaf: u32,
    #[session]
    depth: u32,
    #[session]
    trail: u32,
    #[session]
    notes: Text,
    #[session]
    query: Text,
    #[session]
    title: Text,
    #[session]
    copies: u32,
    #[session]
    volume: u8,
    #[session]
    pin: Text,
    #[session]
    printing: Printing,
    #[session]
    attachment: Text,
    #[session]
    labels: Vec<String>,
    #[session]
    hours: (u8, u8),
    #[session]
    showing: Vec<Folder>,
    #[session]
    city: Text,
    #[session]
    details: bool,
    #[session]
    due: Day,
    #[session]
    opens: Time,
    #[session]
    zone: Text,
    #[session]
    accent: Oklch,
    #[resource]
    hills: Image,
    #[resource]
    rings: PaintHook,
    #[session]
    cities: List<City>,
    #[session]
    picked: Selection<City>,
    #[session]
    sort: Sort,
    #[derived(City::order, from = [cities, sort])]
    order: Derived<Order<City>>,
    #[session]
    looking_for: Text,
    #[derived(City::found, from = [cities, looking_for])]
    found: Derived<Order<City>>,
    #[session]
    narrowed: Text,
    #[derived(City::matching, from = [cities, narrowed])]
    matching: Derived<Order<City>>,
    #[session]
    timeline: TimeView,
    #[session]
    playhead: i64,
    #[session]
    arranged: Sort,
    #[derived(City::arranged, from = [cities, arranged])]
    in_order: Derived<Order<City>>,
    #[session]
    outline: List<Part>,
    #[session]
    reached: Selection<Part>,
    #[session]
    unfolded: Vec<Key<Part>>,
    #[derived(Part::shown, from = [outline, unfolded])]
    shown: Derived<Order<Part>>,
}

/// A row of the live example of a tree: a folder or a file, inside the folder it names.
#[derive(Facts)]
pub struct Part {
    #[key]
    id: Key<Part>,
    name: String,
    inside: Option<Key<Part>>,
}

impl Part {
    fn shown(outline: &List<Part>, unfolded: &[Key<Part>]) -> Order<Part> {
        Order::tree(outline, unfolded, |part| part.inside)
    }
}

const OUTLINE: [(&str, u64); 14] = [
    ("graticule", 0),
    ("apps", 1),
    ("todo", 2),
    ("crates", 1),
    ("kernel", 4),
    ("src", 5),
    ("ledger", 4),
    ("specimen", 4),
    ("docs", 1),
    ("components", 9),
    ("guide", 9),
    ("internals", 9),
    ("spec.md", 9),
    ("README.md", 1),
];

/// A row of the live example of a table.
#[derive(Facts)]
pub struct City {
    #[key]
    id: Key<City>,
    name: String,
    country: String,
    people: u32,
    change: i32,
}

impl City {
    fn arranged(cities: &List<City>, arranged: &Sort) -> Order<City> {
        Order::sorted(cities, arranged)
    }

    fn matching(cities: &List<City>, narrowed: &Text) -> Order<City> {
        Order::of(cities).matching(cities, narrowed.as_str())
    }

    fn found(cities: &List<City>, looking_for: &Text) -> Order<City> {
        Order::found(cities, looking_for.as_str(), |c| &c.name)
    }

    fn order(cities: &List<City>, sort: &Sort) -> Order<City> {
        match sort.column() {
            "City" => Order::by(cities, sort, |c| c.name.clone()),
            "Country" => Order::by(cities, sort, |c| c.country.clone()),
            "People" => Order::by(cities, sort, |c| c.people),
            "Change" => Order::by(cities, sort, |c| c.change),
            _ => Order::of(cities),
        }
    }
}

const BEAT: i64 = 96;

const RAIN: [(&str, f64, f64); 12] = [
    ("Jan", 78.0, 4.1),
    ("Feb", 61.0, 4.6),
    ("Mar", 57.0, 7.4),
    ("Apr", 49.0, 10.2),
    ("May", 55.0, 13.9),
    ("Jun", 47.0, 16.8),
    ("Jul", 51.0, 18.9),
    ("Aug", 59.0, 18.6),
    ("Sep", 54.0, 15.7),
    ("Oct", 73.0, 11.9),
    ("Nov", 81.0, 7.6),
    ("Dec", 84.0, 4.9),
];

fn candles() -> Vec<kernel::Candle> {
    let mut price = 100.0;
    (0..24)
        .map(|k| {
            let swing = ((k * 37 % 11) as f64 - 5.0) * 0.8;
            let open = price;
            price += swing;
            kernel::Candle { open, high: open.max(price) + 1.5, low: open.min(price) - 1.2, close: price }
        })
        .collect()
}

fn trend(city: &City) -> Vec<f64> {
    let base = f64::from(city.people) / 1000.0;
    (0..12).map(|k| base + f64::from(city.change) / 1000.0 * f64::from(k) + ((k * 7 + city.name.len() as i32) % 5) as f64 * base / 200.0).collect()
}

type Clip = (i64, i64, &'static str);

const TRACKS: [(&str, &[Clip]); 3] =
    [("Drums", &[(0, 16, "Groove"), (16, 32, "Groove"), (32, 40, "Fill")]), ("Bass", &[(4, 20, "Walk"), (24, 40, "Walk, higher")]), ("Keys", &[(8, 24, "Chords"), (28, 36, "Stab")])];

const PEOPLE: [(&str, u32, i32); 12] = [
    ("Netherlands", 931_000, 8_200),
    ("Greece", 643_000, -2_100),
    ("Germany", 3_880_000, 21_500),
    ("Portugal", 545_000, 3_900),
    ("United Kingdom", 8_870_000, 61_000),
    ("Spain", 3_330_000, 37_400),
    ("Norway", 717_000, 6_800),
    ("France", 2_100_000, -11_600),
    ("Czechia", 1_380_000, 9_100),
    ("Italy", 2_750_000, -4_300),
    ("Austria", 2_000_000, 19_700),
    ("Poland", 1_860_000, 2_400),
];

const CITIES: [&str; 12] = ["Amsterdam", "Athens", "Berlin", "Lisbon", "London", "Madrid", "Oslo", "Paris", "Prague", "Rome", "Vienna", "Warsaw"];

#[derive(Facts)]
pub struct Section {
    #[key]
    id: Key<Section>,
    name: String,
    component: Component,
}

/// One piece of a component's page: a line of its documentation, or a live example.
#[derive(Facts)]
pub struct Block {
    #[key]
    id: Key<Block>,
    kind: Piece,
    text: String,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Piece {
    #[default]
    Paragraph,
    Heading,
    Bullet,
    Code,
    Live,
    Awkward,
    Icon,
}

/// One line of the Style place: a heading, a sentence, an ink in both appearances, or a value of the style.
#[derive(Facts)]
pub struct Setting {
    #[key]
    id: Key<Setting>,
    shown: Shown,
    name: String,
    light: String,
    dark: String,
}

#[derive(Facts, Copy, Debug, Default, PartialEq)]
pub enum Shown {
    #[default]
    Heading,
    Paragraph,
    Columns,
    Ink(Ink),
    Knob(Knob),
    Value,
}

/// A value of the style, as a line of the Style place shows it.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Knob {
    #[default]
    Lines,
    Shadow,
    Check,
}

/// Content that breaks careless components: nothing, too much, the other direction, other scripts, pictures and long numbers.
#[derive(Facts)]
pub struct Sample {
    #[key]
    id: Key<Sample>,
    what: String,
    text: String,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Component {
    #[default]
    Page,
    Text,
    Button,
    Link,
    Check,
    Field,
    List,
    Empty,
    Swatch,
    Picture,
    Value,
    Row,
    Columns,
    Bar,
    Status,
    Message,
    Palette,
    Heading,
    Separator,
    Keys,
    Progress,
    Meter,
    Placeholder,
    Tag,
    Avatar,
    Callout,
    Card,
    Toggle,
    Group,
    IconButton,
    Switch,
    Radio,
    Fieldset,
    Labelled,
    Tabs,
    Navigation,
    Pagination,
    Disclosure,
    Breadcrumbs,
    PanelStack,
    Overflow,
    ScrollEdges,
    TextArea,
    Search,
    Editable,
    Number,
    Slider,
    CodeField,
    Form,
    Choice,
    FileField,
    TagField,
    RangeSlider,
    Select,
    ComboBox,
    MultiSelect,
    Menu,
    Popover,
    ConfirmRow,
    SidePanel,
    Calendar,
    DateField,
    DatePicker,
    TimeField,
    TimeZone,
    ColorField,
    ColorArea,
    ColorPicker,
    Canvas,
    Image,
    PaintHook,
    Table,
    Tree,
    SplitPanes,
    Picker,
    TypedFilter,
    Records,
    TimeAxis,
    Chart,
}

impl Component {
    const ALL: [Component; 79] = [
        Component::Page,
        Component::Row,
        Component::Columns,
        Component::Text,
        Component::Button,
        Component::Link,
        Component::Check,
        Component::Field,
        Component::List,
        Component::Empty,
        Component::Swatch,
        Component::Picture,
        Component::Value,
        Component::Bar,
        Component::Status,
        Component::Message,
        Component::Palette,
        Component::Heading,
        Component::Separator,
        Component::Keys,
        Component::Progress,
        Component::Meter,
        Component::Placeholder,
        Component::Tag,
        Component::Avatar,
        Component::Callout,
        Component::Card,
        Component::Toggle,
        Component::Group,
        Component::IconButton,
        Component::Switch,
        Component::Radio,
        Component::Fieldset,
        Component::Labelled,
        Component::Tabs,
        Component::Navigation,
        Component::Pagination,
        Component::Disclosure,
        Component::Breadcrumbs,
        Component::PanelStack,
        Component::Overflow,
        Component::ScrollEdges,
        Component::TextArea,
        Component::Search,
        Component::Editable,
        Component::Number,
        Component::Slider,
        Component::CodeField,
        Component::Form,
        Component::Choice,
        Component::FileField,
        Component::TagField,
        Component::RangeSlider,
        Component::Select,
        Component::ComboBox,
        Component::MultiSelect,
        Component::Menu,
        Component::Popover,
        Component::ConfirmRow,
        Component::SidePanel,
        Component::Calendar,
        Component::DateField,
        Component::DatePicker,
        Component::TimeField,
        Component::TimeZone,
        Component::ColorField,
        Component::ColorArea,
        Component::ColorPicker,
        Component::Canvas,
        Component::Image,
        Component::PaintHook,
        Component::Table,
        Component::Tree,
        Component::SplitPanes,
        Component::Picker,
        Component::TypedFilter,
        Component::Records,
        Component::TimeAxis,
        Component::Chart,
    ];

    fn name(self) -> &'static str {
        match self {
            Component::Page => "Page",
            Component::Text => "Text",
            Component::Button => "Button",
            Component::Link => "Link",
            Component::Check => "Check",
            Component::Field => "Text field",
            Component::List => "List",
            Component::Empty => "Empty",
            Component::Swatch => "Swatch",
            Component::Picture => "Picture",
            Component::Value => "Values",
            Component::Row => "Row",
            Component::Columns => "Columns",
            Component::Bar => "Action bar",
            Component::Status => "Status line",
            Component::Message => "Message line",
            Component::Palette => "Palette",
            Component::Heading => "Heading",
            Component::Separator => "Separator",
            Component::Keys => "Key label",
            Component::Progress => "Progress",
            Component::Meter => "Meter",
            Component::Placeholder => "Placeholder",
            Component::Tag => "Tag",
            Component::Avatar => "Avatar",
            Component::Callout => "Callout",
            Component::Card => "Card",
            Component::Toggle => "Toggle button",
            Component::Group => "Button group",
            Component::IconButton => "Icon button",
            Component::Switch => "Switch",
            Component::Radio => "Radio group",
            Component::Fieldset => "Fieldset",
            Component::Labelled => "Label",
            Component::Tabs => "Tabs",
            Component::Navigation => "Navigation",
            Component::Pagination => "Pagination",
            Component::Disclosure => "Disclosure",
            Component::Breadcrumbs => "Breadcrumbs",
            Component::PanelStack => "Panel stack",
            Component::Overflow => "Overflow",
            Component::ScrollEdges => "Scroll edges",
            Component::TextArea => "Text area",
            Component::Search => "Search field",
            Component::Editable => "Editable text",
            Component::Number => "Number field",
            Component::Slider => "Slider",
            Component::CodeField => "Code field",
            Component::Form => "Form",
            Component::Choice => "Choice",
            Component::FileField => "File field",
            Component::TagField => "Tag field",
            Component::RangeSlider => "Range slider",
            Component::Select => "Select",
            Component::ComboBox => "Combo box",
            Component::MultiSelect => "Multi-select",
            Component::Menu => "Menu",
            Component::Popover => "Popover",
            Component::ConfirmRow => "Confirm row",
            Component::SidePanel => "Side panel",
            Component::Calendar => "Calendar",
            Component::DateField => "Date field",
            Component::DatePicker => "Date picker",
            Component::TimeField => "Time field",
            Component::TimeZone => "Time zone",
            Component::ColorField => "Color field",
            Component::ColorArea => "Color area and slider",
            Component::ColorPicker => "Color picker",
            Component::Canvas => "Canvas",
            Component::Image => "Image",
            Component::PaintHook => "Paint hook",
            Component::Table => "Table",
            Component::Tree => "Tree",
            Component::SplitPanes => "Split panes",
            Component::Picker => "Picker",
            Component::TypedFilter => "Typed filter",
            Component::Records => "Table from data",
            Component::TimeAxis => "Time axis",
            Component::Chart => "Chart",
        }
    }

    fn doc(self) -> &'static str {
        match self {
            Component::Page => include_str!("../../../docs/components/page.md"),
            Component::Text => include_str!("../../../docs/components/text.md"),
            Component::Button => include_str!("../../../docs/components/button.md"),
            Component::Link => include_str!("../../../docs/components/link.md"),
            Component::Check => include_str!("../../../docs/components/check.md"),
            Component::Field => include_str!("../../../docs/components/field.md"),
            Component::List => include_str!("../../../docs/components/list.md"),
            Component::Empty => include_str!("../../../docs/components/empty.md"),
            Component::Swatch => include_str!("../../../docs/components/swatch.md"),
            Component::Picture => include_str!("../../../docs/components/picture.md"),
            Component::Value => include_str!("../../../docs/components/value.md"),
            Component::Row => include_str!("../../../docs/components/row.md"),
            Component::Columns => include_str!("../../../docs/components/columns.md"),
            Component::Bar => include_str!("../../../docs/components/bar.md"),
            Component::Status => include_str!("../../../docs/components/status.md"),
            Component::Message => include_str!("../../../docs/components/message.md"),
            Component::Palette => include_str!("../../../docs/components/palette.md"),
            Component::Heading => include_str!("../../../docs/components/heading.md"),
            Component::Separator => include_str!("../../../docs/components/separator.md"),
            Component::Keys => include_str!("../../../docs/components/keys.md"),
            Component::Progress => include_str!("../../../docs/components/progress.md"),
            Component::Meter => include_str!("../../../docs/components/meter.md"),
            Component::Placeholder => include_str!("../../../docs/components/placeholder.md"),
            Component::Tag => include_str!("../../../docs/components/tag.md"),
            Component::Avatar => include_str!("../../../docs/components/avatar.md"),
            Component::Callout => include_str!("../../../docs/components/callout.md"),
            Component::Card => include_str!("../../../docs/components/card.md"),
            Component::Toggle => include_str!("../../../docs/components/toggle.md"),
            Component::Group => include_str!("../../../docs/components/group.md"),
            Component::IconButton => include_str!("../../../docs/components/icon-button.md"),
            Component::Switch => include_str!("../../../docs/components/switch.md"),
            Component::Radio => include_str!("../../../docs/components/radio.md"),
            Component::Fieldset => include_str!("../../../docs/components/fieldset.md"),
            Component::Labelled => include_str!("../../../docs/components/labelled.md"),
            Component::Tabs => include_str!("../../../docs/components/tabs.md"),
            Component::Navigation => include_str!("../../../docs/components/navigation.md"),
            Component::Pagination => include_str!("../../../docs/components/pagination.md"),
            Component::Disclosure => include_str!("../../../docs/components/disclosure.md"),
            Component::Breadcrumbs => include_str!("../../../docs/components/breadcrumbs.md"),
            Component::PanelStack => include_str!("../../../docs/components/panel-stack.md"),
            Component::Overflow => include_str!("../../../docs/components/overflow.md"),
            Component::ScrollEdges => include_str!("../../../docs/components/scroll-edges.md"),
            Component::TextArea => include_str!("../../../docs/components/text-area.md"),
            Component::Search => include_str!("../../../docs/components/search-field.md"),
            Component::Editable => include_str!("../../../docs/components/editable-text.md"),
            Component::Number => include_str!("../../../docs/components/number-field.md"),
            Component::Slider => include_str!("../../../docs/components/slider.md"),
            Component::CodeField => include_str!("../../../docs/components/code-field.md"),
            Component::Form => include_str!("../../../docs/components/form.md"),
            Component::Choice => include_str!("../../../docs/components/choice.md"),
            Component::FileField => include_str!("../../../docs/components/file-field.md"),
            Component::TagField => include_str!("../../../docs/components/tag-field.md"),
            Component::RangeSlider => include_str!("../../../docs/components/range-slider.md"),
            Component::Select => include_str!("../../../docs/components/select.md"),
            Component::ComboBox => include_str!("../../../docs/components/combo-box.md"),
            Component::MultiSelect => include_str!("../../../docs/components/multi-select.md"),
            Component::Menu => include_str!("../../../docs/components/menu.md"),
            Component::Popover => include_str!("../../../docs/components/popover.md"),
            Component::ConfirmRow => include_str!("../../../docs/components/confirm-row.md"),
            Component::SidePanel => include_str!("../../../docs/components/side-panel.md"),
            Component::Calendar => include_str!("../../../docs/components/calendar.md"),
            Component::DateField => include_str!("../../../docs/components/date-field.md"),
            Component::DatePicker => include_str!("../../../docs/components/date-picker.md"),
            Component::TimeField => include_str!("../../../docs/components/time-field.md"),
            Component::TimeZone => include_str!("../../../docs/components/time-zone.md"),
            Component::ColorField => include_str!("../../../docs/components/color-field.md"),
            Component::ColorArea => include_str!("../../../docs/components/color-area.md"),
            Component::ColorPicker => include_str!("../../../docs/components/color-picker.md"),
            Component::Canvas => include_str!("../../../docs/components/canvas.md"),
            Component::Image => include_str!("../../../docs/components/image.md"),
            Component::PaintHook => include_str!("../../../docs/components/paint-hook.md"),
            Component::Table => include_str!("../../../docs/components/table.md"),
            Component::Tree => include_str!("../../../docs/components/tree.md"),
            Component::SplitPanes => include_str!("../../../docs/components/split-panes.md"),
            Component::Picker => include_str!("../../../docs/components/picker.md"),
            Component::TypedFilter => include_str!("../../../docs/components/typed-filter.md"),
            Component::Records => include_str!("../../../docs/components/table-from-data.md"),
            Component::TimeAxis => include_str!("../../../docs/components/time-axis.md"),
            Component::Chart => include_str!("../../../docs/components/chart.md"),
        }
    }

    fn awkward(self) -> bool {
        matches!(self, Component::Text | Component::Button | Component::Link | Component::Check | Component::Value | Component::Heading | Component::Tag | Component::Callout | Component::Card)
    }
}

#[derive(Commands)]
pub enum Cmd {
    #[label("Press")]
    Press,
    #[label("Follow the link")]
    Follow,
    #[label("Tick or untick the box")]
    Tick,
    #[label("Bold")]
    Bold,
    #[label("Go one level in")]
    Deeper,
    #[label("Say it")]
    Say,
    #[label("Open the page for “{name}”", short: "Open")]
    Open(#[subject] Key<Section>),
    #[label("Go to “{name}”", short: "Go")]
    Visit(#[subject] Key<City>),
    #[label("Back to all components")]
    Back,
    #[label("{text}")]
    Echo(#[subject] Key<Sample>),
    #[label("Make “{name}” lighter", short: "Lighter")]
    Lighter(#[subject] Key<Setting>),
    #[label("Make “{name}” darker", short: "Darker")]
    Darker(#[subject] Key<Setting>),
    #[label("Give “{name}” more color", short: "More color")]
    MoreColor(#[subject] Key<Setting>),
    #[label("Give “{name}” less color", short: "Less color")]
    LessColor(#[subject] Key<Setting>),
    #[label("Turn the hue of “{name}” forward", short: "Hue forward")]
    HueForward(#[subject] Key<Setting>),
    #[label("Turn the hue of “{name}” back", short: "Hue back")]
    HueBack(#[subject] Key<Setting>),
    #[label("Change the light appearance")]
    EditLight,
    #[label("Change the dark appearance")]
    EditDark,
    #[label("Use the next weight of lines")]
    NextLines,
    #[label("Deepen the shadow")]
    DeeperShadow,
    #[label("Make the shadow shallower")]
    ShallowerShadow,
    #[label("Use the other check mark")]
    OtherCheck,
    #[label("Save the theme and the style")]
    SaveLook,
    #[internal]
    SavedLook(Result<Path, Failure>),
    #[internal]
    Reached(Reaches),
    #[internal]
    Go(Place),
    #[internal]
    Edit(Edit),
}

const SAMPLES: [(&str, &str); 6] = [
    ("Nothing", ""),
    ("Very long", "A label that goes on and on, well past the edge of any reasonable window, to see where it wraps and what it pushes aside"),
    ("Right to left", "مرحبا بالعالم"),
    ("Chinese", "你好，世界"),
    ("Emoji", "Party 🎉🎈"),
    ("A huge number", "12345678901234567890"),
];

impl Default for Catalog {
    fn default() -> Self {
        let sections = Component::ALL.iter().zip(1..).map(|(&component, n)| Section { id: Key::fixed(n), name: component.name().to_string(), component }).collect();
        let samples = SAMPLES.iter().zip(1..).map(|((what, text), n)| Sample { id: Key::fixed(n), what: what.to_string(), text: text.to_string() }).collect();
        let (lines, shadow, check) = style::start();
        Catalog {
            place: Place::Components,
            sections,
            chosen: Selection::default(),
            page: None,
            blocks: List::default(),
            reading: Selection::default(),
            settings: settings(),
            looking: Selection::default(),
            editing: Side::Light,
            inks: style::mixes(),
            lines,
            shadow,
            check,
            reach: List::default(),
            screens: 0,
            samples,
            draft: Text::default(),
            ticked: false,
            pressed: 0,
            bold: false,
            wrapping: true,
            shape: Shape::List,
            pane: Pane::General,
            folder: Folder::Inbox,
            open: false,
            leaf: 4,
            depth: 0,
            trail: 5,
            notes: Text::from("Milk, eggs and bread.\nAsk about the long weekend before Friday, since the office closes early that week."),
            query: Text::default(),
            title: Text::from("Groceries"),
            copies: 1284,
            volume: 40,
            pin: Text::from("40"),
            printing: Printing::default(),
            attachment: Text::default(),
            labels: vec!["groceries".to_string(), "weekend".to_string()],
            hours: (9, 17),
            showing: vec![Folder::Inbox, Folder::Drafts],
            city: Text::default(),
            details: true,
            due: Day::new(2026, 6, 3).unwrap_or_default(),
            opens: Time::new(9, 0).unwrap_or_default(),
            zone: Text::from("Europe/Lisbon"),
            accent: Oklch::new(0.63, 0.19, 290.0),
            hills: hills(),
            rings: PaintHook::new(crate::rings::Rings::default()),
            cities: CITIES
                .iter()
                .zip(PEOPLE)
                .zip(1..)
                .map(|((name, (country, people, change)), n)| City { id: Key::fixed(n), name: name.to_string(), country: country.to_string(), people, change })
                .collect(),
            picked: Selection::default(),
            sort: Sort::default(),
            order: Derived::default(),
            looking_for: Text::default(),
            found: Derived::default(),
            narrowed: Text::default(),
            matching: Derived::default(),
            timeline: TimeView::new(0, BEAT / 4),
            playhead: BEAT * 4,
            arranged: Sort::default(),
            in_order: Derived::default(),
            outline: OUTLINE.iter().zip(1..).map(|(&(name, inside), n)| Part { id: Key::fixed(n), name: name.to_string(), inside: (inside > 0).then(|| Key::fixed(inside)) }).collect(),
            reached: Selection::default(),
            unfolded: vec![Key::fixed(1), Key::fixed(4)],
            shown: Derived::default(),
        }
    }
}

impl App for Catalog {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Say => kernel::no_if(self.draft.is_empty(), Message::from("Type something first")),
            Cmd::Open(section) => kernel::no_if(!self.sections.has(*section), Message::from("There is no such component")),
            Cmd::Deeper => kernel::no_if(self.depth as usize + 1 >= LEVELS.len(), Message::from("This is as deep as it goes")),
            Cmd::Back => kernel::no_if(self.page.is_none(), Message::from("All components are showing")),
            Cmd::Visit(city) => kernel::no_if(!self.cities.has(*city), Message::from("There is no such city")),
            Cmd::Echo(sample) => kernel::no_if(!self.samples.has(*sample), Message::from("There is no such sample")),
            Cmd::Lighter(s) | Cmd::Darker(s) | Cmd::MoreColor(s) | Cmd::LessColor(s) | Cmd::HueForward(s) | Cmd::HueBack(s) => match self.nudge(*s, nudge(cmd)) {
                Ok(_) => Available::Yes,
                Err(why) => Available::No(why),
            },
            Cmd::EditLight => kernel::no_if(self.editing == Side::Light, Message::from("The light appearance is being changed already")),
            Cmd::EditDark => kernel::no_if(self.editing == Side::Dark, Message::from("The dark appearance is being changed already")),
            Cmd::DeeperShadow => kernel::no_if(self.shadow >= 50, Message::from("The shadow is as deep as it goes, half a row")),
            Cmd::ShallowerShadow => kernel::no_if(self.shadow == 0, Message::from("There is no shadow now")),
            Cmd::SaveLook => kernel::no_if(!self.edited(), Message::from("Nothing has been changed from Graticule's own theme and style")),
            Cmd::Press | Cmd::Follow | Cmd::Tick | Cmd::Bold | Cmd::Go(_) | Cmd::Edit(_) | Cmd::NextLines | Cmd::OtherCheck | Cmd::SavedLook(_) | Cmd::Reached(_) => Available::Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Press => {
                self.pressed += 1;
                let _ = cx.say(Message::plain(format!("The button was pressed {} times", self.pressed)));
                Outcome::Changed
            }
            Cmd::Follow => cx.say(Message::from("A link runs a command, the same as a button")),
            Cmd::Tick => {
                self.ticked = !self.ticked;
                Outcome::Changed
            }
            Cmd::Bold => {
                self.bold = !self.bold;
                Outcome::Changed
            }
            Cmd::Deeper => {
                self.depth += 1;
                Outcome::Changed
            }
            Cmd::Say => {
                let said = self.draft.take();
                let _ = cx.say(Message::plain(format!("You typed “{said}”")));
                Outcome::Changed
            }
            Cmd::Open(section) => {
                let Some(component) = self.sections.find(section).map(|s| s.component) else { return Outcome::Nothing };
                self.page = Some(component);
                let _ = self.blocks.replace(blocks(component).iter().cloned());
                Outcome::Changed
            }
            Cmd::Back => {
                self.page = None;
                let _ = self.blocks.replace([]);
                Outcome::Changed
            }
            Cmd::Visit(city) => {
                let name = self.cities.find(city).map(|c| c.name.clone()).unwrap_or_default();
                cx.say(Message::plain(format!("Went to {name}")))
            }
            Cmd::Echo(sample) => {
                let what = self.samples.find(sample).map(|s| s.what.clone()).unwrap_or_default();
                cx.say(Message::plain(format!("The “{what}” sample ran its command")))
            }
            Cmd::Go(place) if place == self.place => Outcome::Nothing,
            Cmd::Go(place) => {
                self.place = place;
                Outcome::Changed
            }
            Cmd::Lighter(s) | Cmd::Darker(s) | Cmd::MoreColor(s) | Cmd::LessColor(s) | Cmd::HueForward(s) | Cmd::HueBack(s) => {
                let Ok((ink, shade)) = self.nudge(s, nudge(&cmd)) else { return Outcome::Nothing };
                let Some(key) = self.inks.iter().find(|m| m.ink == ink).map(|m| m.id) else { return Outcome::Nothing };
                let side = self.editing;
                self.inks.change(key, |m| if side == Side::Light { m.light = shade } else { m.dark = shade })
            }
            Cmd::EditLight => {
                self.editing = Side::Light;
                Outcome::Changed
            }
            Cmd::EditDark => {
                self.editing = Side::Dark;
                Outcome::Changed
            }
            Cmd::NextLines => {
                self.lines = match self.lines {
                    Weight::Thin => Weight::Regular,
                    Weight::Regular => Weight::Heavy,
                    Weight::Heavy => Weight::Thin,
                };
                Outcome::Changed
            }
            Cmd::DeeperShadow => {
                self.shadow = (self.shadow + 2).min(50);
                Outcome::Changed
            }
            Cmd::ShallowerShadow => {
                self.shadow = self.shadow.saturating_sub(2);
                Outcome::Changed
            }
            Cmd::OtherCheck => {
                self.check = if self.check == Mark::Box { Mark::Tick } else { Mark::Box };
                Outcome::Changed
            }
            Cmd::SaveLook => {
                let folder = kernel::look::folder();
                cx.write(Path(folder.join("graticule.theme")), |c: &Catalog| style::theme_file(&c.inks).print(), Cmd::SavedLook);
                cx.write(Path(folder.join("graticule.style")), |c: &Catalog| style::style_file(c.lines, c.shadow, c.check).print(), Cmd::SavedLook);
                Outcome::Nothing
            }
            Cmd::SavedLook(Ok(path)) => cx.say(Message::plain(format!("Saved {}; every Graticule app draws in it now", path.shortened(60)))),
            Cmd::SavedLook(Err(failure)) => cx.say_failure(failure),
            Cmd::Reached(found) => {
                self.screens = found.screens;
                self.reach.replace(found.inks.iter().cloned())
            }
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        match (self.place, self.page) {
            (Place::Components, None) => self.overview(ui),
            (Place::Components, Some(component)) => self.sheet(ui, component),
            (Place::Screens, _) => ui.page(Message::from("Screens"), |ui| {
                ui.muted("Specimen was opened on its own, so there are no app screens to show. Run an app with --specimen to see its named states here.");
            }),
            (Place::Style, _) => self.style(ui),
            (Place::Changes, _) => ui.page(Message::from("Changes"), |ui| {
                ui.muted("Changes come in a later part of this item.");
            }),
        }
    }

    fn states() -> States<Self> {
        let mut chosen = Catalog { place: Place::Style, ..Catalog::default() };
        if let Some(link) = chosen.settings.iter().find(|s| s.shown == Shown::Ink(Ink::Link)).map(|s| s.id) {
            let _ = chosen.looking.choose(&ledger::Choose::Only(link.raw()));
        }
        let start = States::new().add("Components", Catalog::default()).add("Style", Catalog { place: Place::Style, ..Catalog::default() }).add("Style, an ink chosen", chosen);
        Component::ALL.iter().fold(start, |states, &component| states.add(component.name(), Catalog { page: Some(component), blocks: blocks(component), ..Catalog::default() }))
    }
}

impl Catalog {
    fn overview(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Components"), |ui| {
            ui.muted("Every component in its default form, live: click it, type into it, use it from the keyboard. Open one for its page.");
            ui.list(&self.sections, Catalog::chosen, |ui, section| {
                ui.columns(|ui| {
                    ui.column(14, |ui| ui.text(&section.name));
                    ui.column(kernel::Fill, |ui| self.example(ui, section.component));
                    ui.column(6, |ui| {
                        ui.link(Cmd::Open);
                    });
                });
            });
        });
    }

    fn style(&self, ui: &mut Ui<Cmd>) {
        let file = style::theme_file(&self.inks);
        let (light, dark) = (file.theme(Appearance::Light), file.theme(Appearance::Dark));
        let changing = |side: Side, name: &str| if self.editing == side { format!("{name}, being changed") } else { name.to_string() };
        let reach = |ink: Ink| match self.reach.iter().find(|r| r.ink == ink) {
            Some(r) => format!("on {} of {} screens", r.screens, self.screens),
            None => String::new(),
        };
        ui.page(Message::from("Style"), |ui| {
            ui.list(&self.settings, Catalog::looking, |ui, setting| match setting.shown {
                Shown::Heading => ui.heading(&setting.name),
                Shown::Paragraph => ui.muted(&setting.name),
                Shown::Columns => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN, |ui| ui.muted("Now"));
                    ui.column(NAME_COLUMN, |ui| ui.muted(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.muted(changing(Side::Light, &setting.light)));
                    ui.column(kernel::Fill, |ui| ui.muted(changing(Side::Dark, &setting.dark)));
                    ui.column(REACH_COLUMN, |ui| ui.muted("Drawn"));
                }),
                Shown::Ink(ink) => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN, |ui| ui.swatch(ink));
                    ui.column(NAME_COLUMN, |ui| ui.text(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.mono(describe(&light, style::shade(&self.inks, ink, Side::Light), ink)));
                    ui.column(kernel::Fill, |ui| ui.mono(describe(&dark, style::shade(&self.inks, ink, Side::Dark), ink)));
                    ui.column(REACH_COLUMN, |ui| ui.muted(reach(ink)));
                }),
                Shown::Knob(knob) => ui.columns(|ui| {
                    let value = match knob {
                        Knob::Lines => format!("{:?}", self.lines).to_lowercase(),
                        Knob::Shadow if self.shadow == 0 => "none".to_string(),
                        Knob::Shadow => format!("{:.2} of a row", self.shadow as f32 / 100.0),
                        Knob::Check => format!("{:?}", self.check).to_lowercase(),
                    };
                    ui.column(SWATCH_COLUMN + NAME_COLUMN + GAP, |ui| ui.text(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.mono(value));
                    ui.column(kernel::Fill, |ui| ui.muted(&setting.dark));
                }),
                Shown::Value => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN + NAME_COLUMN + GAP, |ui| ui.text(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.mono(&setting.light));
                    ui.column(kernel::Fill, |ui| ui.muted(&setting.dark));
                }),
            });
        });
    }

    fn sheet(&self, ui: &mut Ui<Cmd>, component: Component) {
        ui.page(Message::plain(component.name()), |ui| {
            ui.list(&self.blocks, Catalog::reading, |ui, block| match block.kind {
                Piece::Heading => ui.heading(&block.text),
                Piece::Paragraph => ui.text(&block.text),
                Piece::Bullet => ui.text(format!("• {}", block.text)),
                Piece::Code => ui.mono(&block.text),
                Piece::Live => ui.columns(|ui| {
                    ui.column(14, |ui| ui.muted("Live"));
                    ui.column(kernel::Fill, |ui| self.example(ui, component));
                }),
                Piece::Icon => {
                    let Some((name, icon)) = kernel::icons::ALL.iter().find(|(name, _)| *name == block.text) else { return };
                    ui.columns(|ui| {
                        ui.column(14, |ui| ui.mono(name));
                        ui.column(kernel::Fill, |ui| {
                            ui.row(|ui| {
                                ui.picture(icon);
                                ui.picture(icon).height(2);
                                ui.muted(icon.label);
                            })
                        });
                    });
                }
                Piece::Awkward => {
                    let Some(sample) = self.samples.iter().find(|s| s.what == block.text) else { return };
                    ui.columns(|ui| {
                        ui.column(14, |ui| ui.muted(&sample.what));
                        ui.column(kernel::Fill, |ui| awkward(ui, component, sample));
                    });
                }
            });
        });
    }

    fn example<S: Copy + 'static>(&self, ui: &mut Ui<Cmd, S>, component: Component) {
        match component {
            Component::Page => ui.muted("This whole place is one"),
            Component::Text => ui.row(|ui| {
                ui.text("Plain text");
                ui.mono("mono");
                ui.muted("muted");
            }),
            Component::Button => {
                ui.button(Cmd::Press);
            }
            Component::Link => {
                ui.link(Cmd::Follow);
            }
            Component::Check => {
                ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
            }
            Component::Field => ui.row(|ui| {
                ui.field(Catalog::draft, Message::from("Type here")).on_submit(Cmd::Say);
                ui.button(Cmd::Say);
            }),
            Component::List => ui.muted("This page is one: the arrows move through it, Shift with them chooses several"),
            Component::Empty => ui.muted("What a list shows when it has nothing, with the command that fills it"),
            Component::Row => ui.row(|ui| {
                ui.text("Side");
                ui.text("by");
                ui.text("side");
            }),
            Component::Columns => ui.columns(|ui| {
                ui.column(10, |ui| ui.text("A column"));
                ui.column(kernel::Fill, |ui| ui.muted("and one that fills the rest"));
            }),
            Component::Bar => ui.muted("At the bottom of this window"),
            Component::Status => ui.muted("Just above the bar: hover over something"),
            Component::Message => ui.muted("Under the page: press the button above"),
            Component::Palette => ui.muted("Open it from the bar"),
            Component::Picture => ui.row(|ui| {
                ui.picture(&kernel::icons::WARNING);
                ui.text("One row tall, beside text");
                ui.picture(&kernel::icons::STAR).height(3);
            }),
            Component::Value => {
                ui.row(|ui| {
                    ui.count(1284);
                    ui.date(kernel::Moment(1_780_308_000_000));
                    ui.version("0.4.1");
                    ui.state("offline", Ink::Danger);
                });
                ui.row(|ui| {
                    ui.product("DK-01");
                    ui.code("is:unread");
                    ui.mode("rows");
                });
                ui.error("Couldn't reach the server");
            }
            Component::Heading => ui.heading("Recent changes"),
            Component::Separator => {
                ui.text("Above the rule");
                ui.separator();
                ui.text("Below the rule");
            }
            Component::Keys => ui.row(|ui| {
                ui.text("Press");
                ui.keys(Cmd::Press);
                ui.text("to press the button");
            }),
            Component::Progress => ui.progress(37, 120),
            Component::Meter => ui.meter(0.62),
            Component::Placeholder => ui.placeholder(3),
            Component::Tag => ui.row(|ui| {
                ui.tag("design");
                ui.tag("urgent");
            }),
            Component::Avatar => ui.row(|ui| {
                ui.avatar("Ada Lovelace");
                ui.text("Ada Lovelace");
            }),
            Component::Callout => {
                ui.callout(Ink::Warn, |ui| {
                    ui.text("The disk is nearly full. Old versions are removed first.");
                });
                ui.callout(Ink::Link, |ui| {
                    ui.text("Changes are saved as you make them.");
                });
            }
            Component::Card => ui.card("Account", |ui| {
                ui.text("ada@example.org");
                ui.button(Cmd::Press);
            }),
            Component::Toggle => {
                ui.toggle(self.bold, Cmd::Bold);
            }
            Component::Group => {
                ui.group(|ui| {
                    ui.button(Cmd::Press);
                    ui.toggle(self.bold, Cmd::Bold);
                });
                ui.segments("Show as", Catalog::shape);
            }
            Component::IconButton => {
                ui.icon_button(&kernel::icons::CLOSE, Cmd::Back);
            }
            Component::Switch => ui.switch("Wrap long lines", Catalog::wrapping),
            Component::Radio => ui.radio("Show as", Catalog::shape),
            Component::Fieldset => ui.fieldset("Writing", |ui| {
                ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
                ui.switch("Wrap long lines", Catalog::wrapping);
            }),
            Component::Labelled => ui.labelled("Something to say", |ui| {
                ui.field(Catalog::draft, Message::from("Type here")).on_submit(Cmd::Say);
                ui.muted("It shows in the message line");
            }),
            Component::Tabs => ui.tabs(Catalog::pane, |ui, pane| match pane {
                Pane::General => ui.switch("Wrap long lines", Catalog::wrapping),
                Pane::Appearance => ui.radio("Show as", Catalog::shape),
                Pane::Keys => ui.muted("Keys are bound in the keymap file"),
            }),
            Component::Navigation => ui.places(Catalog::folder),
            Component::Pagination => ui.pages(Catalog::leaf, 12),
            Component::Disclosure => ui.disclosure("More settings", Catalog::open, |ui| {
                ui.switch("Wrap long lines", Catalog::wrapping);
                ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
            }),
            Component::Breadcrumbs => ui.breadcrumbs(Catalog::trail, &TRAIL),
            Component::PanelStack => {
                ui.breadcrumbs(Catalog::depth, &LEVELS);
                ui.muted(match self.depth {
                    0 => "Everything you can change",
                    1 => "Who you are here",
                    _ => "How you sign in",
                });
                ui.button(Cmd::Deeper);
            }
            Component::Overflow => ui.overflow(|ui| {
                ui.button(Cmd::Press);
                ui.toggle(self.bold, Cmd::Bold);
                ui.button(Cmd::Tick);
                ui.link(Cmd::Follow);
                ui.button(Cmd::Say);
            }),
            Component::ScrollEdges => ui.muted("This page's list shows them: scroll it, and a rule marks the edge with more beyond"),
            Component::TextArea => {
                ui.text_area(Catalog::notes, Message::from("A few lines"), 3);
            }
            Component::Search => {
                ui.search(Catalog::query, Message::from("Words to look for"));
            }
            Component::Editable => {
                ui.editable(Catalog::title, Message::from("Untitled"));
            }
            Component::Number => ui.labelled("Copies", |ui| ui.number(Catalog::copies)),
            Component::Slider => ui.labelled("Volume", |ui| ui.slider(Catalog::volume, 0..=100)),
            Component::CodeField => ui.labelled("Code", |ui| {
                ui.code_field(Catalog::pin, 6);
            }),
            Component::Form => ui.form(Catalog::printing),
            Component::Choice => ui.choice("Show as", Catalog::shape),
            Component::FileField => ui.file_field("Attachment", Catalog::attachment),
            Component::TagField => ui.tag_field("Tags", Catalog::labels),
            Component::RangeSlider => ui.labelled("Opening hours", |ui| ui.range_slider(Catalog::hours, 0..=24)),
            Component::Select => ui.select("Move to", Catalog::folder),
            Component::ComboBox => ui.combo_box(Catalog::city, Message::from("City"), &CITIES),
            Component::MultiSelect => ui.multi_select("Show", Catalog::showing),
            Component::Menu => ui.menu("Edit", |ui| {
                ui.button(Cmd::Press);
                ui.button(Cmd::Bold);
                ui.button(Cmd::Say);
            }),
            Component::Popover => ui.popover("Rename", |ui| {
                ui.field(Catalog::title, Message::from("Title"));
                ui.button(Cmd::Press);
            }),
            Component::ConfirmRow => ui.confirm(Cmd::Press, Message::from("Press it? This press can't be taken back.")),
            Component::SidePanel => {
                ui.switch("Show the details", Catalog::details);
                ui.columns(|ui| {
                    ui.column(kernel::Fill, |ui| {
                        ui.text("Groceries");
                        ui.muted("Milk, eggs and bread.");
                    });
                    if self.details {
                        ui.column(28, |ui| {
                            ui.text("Details");
                            ui.muted("Two tags, made on Friday");
                        });
                    }
                });
            }
            Component::Calendar => ui.calendar("Due", Catalog::due),
            Component::DateField => ui.labelled("Due", |ui| ui.date_field(Catalog::due)),
            Component::DatePicker => ui.date_picker("Due", Catalog::due),
            Component::TimeField => ui.labelled("Opens at", |ui| ui.time_field(Catalog::opens)),
            Component::TimeZone => ui.time_zone(Catalog::zone, Message::from("Time zone")),
            Component::ColorField => ui.labelled("Accent", |ui| ui.color_field(Catalog::accent)),
            Component::ColorArea => {
                ui.color_area(Catalog::accent);
                ui.color_slider(Catalog::accent, Channel::Lightness);
                ui.color_slider(Catalog::accent, Channel::Chroma);
                ui.color_slider(Catalog::accent, Channel::Hue);
            }
            Component::ColorPicker => ui.color_picker(Catalog::accent),
            Component::Chart => {
                let rain = Plot::new(6)
                    .bars(RAIN.iter().enumerate().map(|(k, m)| (k as f64, m.1)), Ink::Selection)
                    .line(RAIN.iter().enumerate().map(|(k, m)| (k as f64, m.2 * 4.0)), Ink::Link)
                    .names(RAIN.iter().map(|m| m.0));
                ui.chart("Rain in millimetres, and four times the warmth in degrees, by month", &rain);
                ui.chart("A price, day by day", &Plot::new(6).candles(candles()));
            }
            Component::TimeAxis => ui.time_axis(Clock::Beats { per_beat: BEAT, beats_per_bar: 4 }, Catalog::timeline, Catalog::playhead, |axis| {
                for (name, clips) in TRACKS {
                    axis.track(name, 2, |lane| {
                        for &(from, to, clip) in clips {
                            let (from, to) = (from * BEAT, to * BEAT);
                            if lane.shows(from, to) {
                                lane.span(from, to).ink(Ink::Selection).fill();
                                lane.span(from, to).ink(Ink::Muted).stroke(kernel::Weight::Line);
                                lane.label(from + BEAT / 4, clip);
                            }
                        }
                    });
                }
            }),
            Component::Records => {
                ui.records(self.in_order.of(&self.cities), Catalog::picked).sort(Catalog::arranged).height(6);
            }
            Component::TypedFilter => {
                ui.filter(&self.cities, Catalog::narrowed);
                ui.table(self.matching.of(&self.cities), Catalog::picked, |row, city| {
                    row.cell("City", &city.name);
                    row.cell("Country", &city.country);
                    row.cell("People", city.people);
                })
                .height(6);
            }
            Component::Picker => {
                ui.picker(self.found.of(&self.cities), Catalog::looking_for, |ui, city| {
                    ui.text(&city.name);
                    ui.muted(&city.country);
                })
                .on_choose(Cmd::Visit);
            }
            Component::SplitPanes => ui.split(|ui| {
                ui.pane("Cities", kernel::cols(22).min(14), |ui| {
                    ui.list(&self.cities, Catalog::picked, |ui, city| ui.text(&city.name)).height(8);
                });
                ui.pane("City", kernel::Fill.min(20), |ui| match self.picked.focused().and_then(|k| self.cities.find(k)) {
                    Some(city) => {
                        ui.text(&city.name);
                        ui.muted(format!("{}, {} people", city.country, kernel::value::grouped(city.people)));
                    }
                    None => ui.muted("Choose a city to see it here"),
                });
            }),
            Component::Tree => {
                ui.tree(self.shown.of(&self.outline), Catalog::reached, Catalog::unfolded, |ui, part| ui.text(&part.name)).height(8);
            }
            Component::Table => {
                ui.table(self.order.of(&self.cities), Catalog::picked, |row, city| {
                    row.cell("City", &city.name);
                    row.cell("Country", &city.country);
                    row.cell("People", city.people);
                    row.cell("Change", city.change).tone_by_sign();
                    row.spark("Trend", &trend(city));
                })
                .sort(Catalog::sort)
                .height(8);
            }
            Component::PaintHook => ui.canvas("Rings painted by a shader", 8, |c| {
                c.paint(&self.rings);
                c.text((0.02, 0.15), "painted on the GPU");
            }),
            Component::Image => ui.image("Green hills under an evening sky", &self.hills, 6),
            Component::Canvas => ui.canvas("Visits this week", 8, |c| {
                const LATEST: u64 = 100;
                let visits = [32.0, 45.0, 41.0, 60.0, 72.0, 58.0, 80.0];
                let points: Vec<(f32, f32)> = visits.iter().enumerate().map(|(k, v)| (k as f32 + 0.5, *v)).collect();
                c.space(0.0..7.0, 100.0..0.0);
                for x in 1..7 {
                    c.line((x as f32, 0.0), (x as f32, 84.0)).ink(Ink::Rule).stroke(kernel::Weight::Hair);
                }
                c.polygon([(0.5, 0.0)].into_iter().chain(points.iter().copied()).chain([(6.5, 0.0)])).ink(Ink::Selection);
                c.polyline(points.iter().copied()).ink(Ink::Link).stroke(kernel::Weight::Bold);
                for &p in &points[..6] {
                    c.dot(p).ink(Ink::Link);
                }
                c.dot(points[6]).ink(Ink::Ok).key(LATEST);
                c.press(LATEST, Cmd::Say);
                c.text((0.2, 92.0), "80 visits on Sunday");
            }),
            Component::Swatch => ui.row(|ui| {
                ui.swatch(Ink::Link);
                ui.swatch(Ink::Danger);
                ui.swatch(Ink::Ok);
                ui.swatch(Ink::Selection);
            }),
        }
    }
}

fn awkward<S: Copy + 'static>(ui: &mut Ui<Cmd, S>, component: Component, sample: &Sample) {
    match component {
        Component::Text => ui.text(&sample.text),
        Component::Button => {
            ui.button(Cmd::Echo(sample.id));
        }
        Component::Link => {
            ui.link(Cmd::Echo(sample.id));
        }
        Component::Check => {
            ui.check(&sample.text, false, Cmd::Echo(sample.id));
        }
        Component::Value => ui.row(|ui| {
            ui.state(&sample.text, Ink::Ok);
            ui.code(&sample.text);
        }),
        Component::Heading => ui.heading(&sample.text),
        Component::Tag => ui.tag(&sample.text),
        Component::Callout => ui.callout(Ink::Danger, |ui| ui.text(&sample.text)),
        Component::Card => ui.card(&sample.text, |ui| ui.text(&sample.text)),
        _ => {}
    }
}

/// A component's page: its documentation, line by line, with the live example after the code and, for components that show text, the awkward samples at the end.
/// A picture of hills under an evening sky, made from numbers so the repository holds no photograph.
fn hills() -> Image {
    let (w, h) = (480u32, 270u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let near = 0.78 + 0.06 * (u * 9.0).sin() + 0.03 * (u * 23.0 + 1.0).sin();
            let far = 0.6 + 0.08 * (u * 5.0 + 2.0).sin();
            let sun = (((u - 0.7) * 16.0 / 9.0).powi(2) + (v - 0.42).powi(2)).sqrt();
            let color = if v > near {
                kernel::ink::oklch(0.55 - (v - near) * 0.5, 0.12, 140.0)
            } else if v > far {
                kernel::ink::oklch(0.45 + (v - far) * 0.3, 0.08, 160.0)
            } else if sun < 0.06 {
                kernel::ink::oklch(0.95, 0.08, 90.0)
            } else {
                kernel::ink::oklch(0.55 + v * 0.35, 0.1 + v * 0.05, 30.0 + v * 50.0)
            };
            rgba.extend(color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    Image::from_rgba(w, h, rgba).unwrap_or_default()
}

fn blocks(component: Component) -> List<Block> {
    let mut pieces: Vec<(Piece, String)> = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |pieces: &mut Vec<(Piece, String)>, paragraph: &mut Vec<&str>| {
        if !paragraph.is_empty() {
            pieces.push((Piece::Paragraph, paragraph.join(" ").replace('`', "")));
            paragraph.clear();
        }
    };
    for line in component.doc().lines().skip(1) {
        if let Some(lines) = &mut code {
            if line.starts_with("```") {
                pieces.push((Piece::Code, lines.join("\n")));
                code = None;
            } else {
                lines.push(line);
            }
        } else if line.starts_with("```") {
            flush(&mut pieces, &mut paragraph);
            code = Some(Vec::new());
        } else if let Some(heading) = line.strip_prefix("## ") {
            flush(&mut pieces, &mut paragraph);
            pieces.push((Piece::Paragraph, String::new()));
            pieces.push((Piece::Heading, heading.to_string()));
        } else if let Some(bullet) = line.strip_prefix("- ") {
            flush(&mut pieces, &mut paragraph);
            pieces.push((Piece::Bullet, bullet.replace('`', "")));
        } else if line.trim().is_empty() {
            flush(&mut pieces, &mut paragraph);
        } else {
            paragraph.push(line.trim());
        }
    }
    flush(&mut pieces, &mut paragraph);
    let after = pieces.len().min(1);
    pieces.splice(after..after, [(Piece::Paragraph, String::new()), (Piece::Live, String::new())]);
    if component == Component::Picture {
        pieces.push((Piece::Paragraph, String::new()));
        pieces.push((Piece::Heading, "The built-in icons".to_string()));
        pieces.extend(kernel::icons::ALL.iter().map(|(name, _)| (Piece::Icon, name.to_string())));
    }
    if component.awkward() {
        pieces.push((Piece::Paragraph, String::new()));
        pieces.push((Piece::Heading, "With awkward content".to_string()));
        pieces.extend(SAMPLES.iter().map(|(what, _)| (Piece::Awkward, what.to_string())));
    }
    pieces.into_iter().zip(1..).map(|((kind, text), n)| Block { id: Key::fixed(n), kind, text }).collect()
}

const SWATCH_COLUMN: i32 = 6;
const NAME_COLUMN: i32 = 12;
const REACH_COLUMN: i32 = 20;

/// The Style place's lines: every ink of the theme in both appearances, the values of the style, then the values the grid and the type are built from. The inks' and the style's numbers come from the facts being edited, when the view draws them.
fn settings() -> List<Setting> {
    let line = |shown: Shown, name: &str, light: &str, dark: &str| (shown, name.to_string(), light.to_string(), dark.to_string());
    let mut lines = vec![
        line(Shown::Heading, "Inks", "", ""),
        line(
            Shown::Paragraph,
            "Everything is drawn in one of these inks, so a theme restyles everything at once. Each is given as OKLCH lightness, chroma and hue, then, for inks that text and lines are drawn in, its contrast on the background: text needs 4.5 to 1, faint text 3 to 1. Choose an ink to change it from the bar; a change that would make text hard to read can't be made, and says why. Every screen and component follows as it moves.",
            "",
            "",
        ),
        line(Shown::Columns, "Ink", "Light", "Dark"),
    ];
    lines.extend(Ink::ALL.into_iter().map(|ink| line(Shown::Ink(ink), ink.name(), "", "")));
    lines.extend([
        line(Shown::Paragraph, "", "", ""),
        line(Shown::Heading, "The style", "", ""),
        line(Shown::Paragraph, "How things are drawn, never where. Saving writes the theme and the style to the looks folder, where every Graticule app finds them.", "", ""),
        line(Shown::Knob(Knob::Lines), "Lines", "", "The edges of boxes and rules: thin, regular or heavy"),
        line(Shown::Knob(Knob::Shadow), "Shadow", "", "The hard shadow under what you can act on, as a share of a row"),
        line(Shown::Knob(Knob::Check), "Check mark", "", "What a ticked box shows"),
        line(Shown::Paragraph, "", "", ""),
        line(Shown::Heading, "The grid and the type", "", ""),
        line(Shown::Paragraph, "These are the same in every style, because the grid is built from them.", "", ""),
    ]);
    let values = [
        ("Type size", format!("{FONT_POINTS} pt"), "The monospace face at this size sets a cell's width, rounded to a whole point"),
        ("Line height", format!("{LINE}"), "A cell is this many lines of type tall, rounded to a whole point"),
        ("Text face", "IBM Plex Sans".to_string(), "For words"),
        ("Data face", "IBM Plex Mono".to_string(), "For data, code and anything that lines up in cells"),
        ("Strong weight", "600".to_string(), "Text that heads what follows it; other text is 400"),
        ("Gap", format!("{GAP} cells"), "Between things side by side"),
        ("Margin", format!("{MARGIN} cells"), "Around a page"),
    ];
    lines.extend(values.iter().map(|(name, value, what)| line(Shown::Value, name, value, what)));
    lines.into_iter().zip(1..).map(|((shown, name, light, dark), n)| Setting { id: Key::fixed(n), shown, name, light, dark }).collect()
}

/// An ink's numbers in one appearance, and its contrast on the background for inks that text and lines are drawn in.
fn describe(theme: &kernel::ink::Theme, shade: style::Shade, ink: Ink) -> String {
    let o = shade.oklch();
    let numbers = format!("{:.3} {:.3} {:.0}", o.l, o.c, o.h);
    if ink.surface() {
        return numbers;
    }
    let ratio = contrast(theme.get(ink), theme.bg);
    match ink.needs() {
        Some(needs) if ratio < needs => format!("{numbers} · {ratio:.1}, below {needs}"),
        _ => format!("{numbers} · {ratio:.1}"),
    }
}

fn nudge(cmd: &Cmd) -> Nudge {
    match cmd {
        Cmd::Darker(_) => Nudge::Darker,
        Cmd::MoreColor(_) => Nudge::MoreColor,
        Cmd::LessColor(_) => Nudge::LessColor,
        Cmd::HueForward(_) => Nudge::HueForward,
        Cmd::HueBack(_) => Nudge::HueBack,
        _ => Nudge::Lighter,
    }
}

impl Catalog {
    fn nudge(&self, setting: Key<Setting>, by: Nudge) -> Result<(Ink, style::Shade), Message> {
        let Some(Shown::Ink(ink)) = self.settings.find(setting).map(|s| s.shown) else { return Err(Message::from("Only an ink has a color to change")) };
        style::nudged(&self.inks, ink, self.editing, by).map(|shade| (ink, shade))
    }

    /// Whether the theme or the style differs from Graticule's own.
    pub fn edited(&self) -> bool {
        style::theme_file(&self.inks) != kernel::look::THEMES[0] || style::style(self.lines, self.shadow, self.check) != kernel::look::STYLES[0].style
    }

    /// The look being edited, in an appearance.
    pub fn edited_look(&self, appearance: Appearance) -> kernel::look::Look {
        kernel::look::Look { theme: style::theme_file(&self.inks).theme(appearance), style: style::style(self.lines, self.shadow, self.check), appearance }
    }
}

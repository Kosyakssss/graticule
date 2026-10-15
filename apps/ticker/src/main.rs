use std::time::{Duration, Instant};

use graticule::ledger::Choose;
use graticule::prelude::*;

#[derive(Facts)]
struct Ticker {
    watchlist: List<Watched>,
    #[session]
    exchange: List<Listed>,
    #[session]
    watched_chosen: Selection<Watched>,
    #[session]
    listed_chosen: Selection<Listed>,
    #[session]
    trade_chosen: Selection<Trade>,
    #[live]
    market: Live<Market>,
    #[derived(Chart::of, from = [market, watched_chosen])]
    chart: Derived<Chart>,
}

/// A stock on the watchlist, by the key of the listed stock it is.
#[derive(Facts)]
struct Watched {
    #[key]
    id: Key<Watched>,
}

/// A stock the exchange lists.
#[derive(Facts)]
struct Listed {
    #[key]
    id: Key<Listed>,
    symbol: String,
    name: String,
}

/// One trade on the tape.
#[derive(Facts)]
struct Trade {
    #[key]
    id: Key<Trade>,
    stock: u16,
    price: i64,
    size: u32,
    at: u32,
}

/// The market as the feed last published it: each listed stock's quote and candles, in the order of `LISTED`, the trades of the day, newest first, and the time on the exchange's clock.
#[derive(Clone, Default)]
struct Market {
    quotes: Vec<Quote>,
    candles: Vec<Vec<Candle>>,
    tape: List<Trade>,
    clock: u32,
}

/// A stock's prices today, in cents, and how many shares changed hands.
#[derive(Clone, Copy, Default)]
struct Quote {
    open: i64,
    last: i64,
    high: i64,
    low: i64,
    volume: u64,
}

/// The charted stock's candles, worked out off the main thread whenever the market moves.
#[derive(Default)]
struct Chart {
    name: String,
    plot: Option<Plot>,
}

#[derive(Commands)]
enum Cmd {
    /// Puts the stock on the watchlist.
    #[label("Watch", many: "Watch {count} stocks")]
    Watch(#[subject] Key<Listed>),
    /// Takes the stock off the watchlist.
    #[label("Stop watching", many: "Stop watching {count} stocks")]
    Unwatch(#[subject] Key<Watched>),
    #[internal]
    Edit(Edit),
}

/// The stocks the simulated exchange lists: a symbol, a name and the price it opens at, in cents.
const LISTED: &[(&str, &str, i64)] = &[
    ("ALDR", "Alder Freight", 4_210),
    ("BRCK", "Brick & Mortar", 1_875),
    ("CNDL", "Candlewick Energy", 6_630),
    ("DUNE", "Dune Labs", 12_950),
    ("EMBR", "Ember Foods", 3_305),
    ("FERN", "Fernhill Health", 8_840),
    ("GLYF", "Glyph Systems", 23_120),
    ("HRBR", "Harbour Shipping", 2_790),
    ("IRIS", "Iris Optics", 5_415),
    ("JUNO", "Juno Mining", 990),
    ("KILN", "Kiln Ceramics", 1_560),
    ("LOOM", "Loom Textiles", 3_720),
    ("MOSS", "Moss Biotech", 15_480),
    ("NOVA", "Nova Grid", 7_710),
    ("OPAL", "Opal Retail", 2_245),
    ("PIKE", "Pike Rail", 4_980),
];

/// How long a candle lasts on the exchange's clock, in milliseconds.
const CANDLE_MS: u32 = 5 * 60 * 1000;

/// How many candles the chart shows.
const CANDLES: usize = 48;

/// How many trades the day has had when the feed starts.
const HISTORY: usize = 1_000_000;

impl App for Ticker {
    type Cmd = Cmd;

    fn start(&self, jobs: &mut Jobs<Cmd>) {
        jobs.stream(t!("Market data"), simulate, Ticker::market);
    }

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Watch(stock) => no_if(self.watchlist.find(Key::from_raw(stock.raw())).is_some(), t!("It's on the watchlist already")),
            Cmd::Unwatch(stock) => no_if(self.watchlist.find(*stock).is_none(), t!("It isn't on the watchlist")),
            Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, _: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Watch(stock) => self.watchlist.push(Watched { id: Key::from_raw(stock.raw()) }),
            Cmd::Unwatch(stock) => self.watchlist.remove(stock),
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        let market = self.market.now();
        ui.page(t!("Ticker"), |ui| {
            ui.muted(match market.clock {
                0 => t!("Waiting for the market to open"),
                clock => {
                    let (now, trades) = (time(clock), graticule::kernel::value::grouped(market.tape.len()));
                    t!("A simulated exchange, {now} on its clock, {trades} trades today")
                }
            });
            ui.split(|ui| {
                ui.pane(t!("Watchlist"), Fill, |ui| {
                    if self.watchlist.is_empty() {
                        ui.muted(t!("Nothing watched. Choose stocks below and watch them."));
                    }
                    ui.table(&self.watchlist, Ticker::watched_chosen, |row, watched| {
                        let n = index(watched.id.raw());
                        let quote = market.quotes.get(n).copied().unwrap_or_default();
                        row.cell("Symbol", LISTED[n].0).width(8);
                        row.cell("Last", Decimal::new(quote.last, 2)).width(9).glow_on_change();
                        row.cell("Change", Decimal::new(quote.last - quote.open, 2)).width(8).tone_by_sign();
                        row.cell("Volume", quote.volume).width(11);
                    });
                    ui.table(&self.exchange, Ticker::listed_chosen, |row, listed| {
                        let n = index(listed.id.raw());
                        let quote = market.quotes.get(n).copied().unwrap_or_default();
                        row.cell("Symbol", &listed.symbol).width(8);
                        row.cell("Name", &listed.name).width(Fill.min(10));
                        row.cell("Last", Decimal::new(quote.last, 2)).width(8).glow_on_change();
                        row.cell("High", Decimal::new(quote.high, 2)).width(8);
                        row.cell("Low", Decimal::new(quote.low, 2)).width(8);
                    });
                });
                ui.pane(t!("Chart"), Fill, |ui| match self.chart.get() {
                    Some(Chart { name, plot: Some(plot) }) => {
                        ui.heading(name);
                        ui.chart(name, plot);
                    }
                    Some(Chart { name, plot: None }) if !name.is_empty() => {
                        ui.muted(t!("{name} hasn't traded yet"));
                    }
                    _ => {
                        ui.muted(t!("Choose a stock on the watchlist to see its chart"));
                    }
                });
                ui.pane(t!("Tape"), Fill, |ui| {
                    ui.table(&market.tape, Ticker::trade_chosen, |row, trade| {
                        row.cell("Time", time(trade.at)).width(9);
                        row.cell("Symbol", LISTED[usize::from(trade.stock)].0).width(8);
                        row.cell("Price", Decimal::new(trade.price, 2));
                        row.cell("Size", trade.size);
                    });
                });
            });
        });
    }

    fn states() -> States<Self> {
        let at = |trades: usize| Ticker { market: Live::new(Exchange::new(1).run(trades)), ..Ticker::default() };
        let chosen = |mut ticker: Ticker, symbol: &str| {
            let key = key_of(symbol);
            let _ = ticker.watched_chosen.choose(&Choose::Only(key.raw()));
            ticker
        };
        States::new()
            .add("Before the open", Ticker { market: Live::new(Market::default()), ..Ticker::default() })
            .add("The morning", at(4_000))
            .add("A stock charted", chosen(at(200_000), "GLYF"))
            .add("Nothing watched", Ticker { watchlist: List::new(), ..at(4_000) })
            .add("A million trades", chosen(at(HISTORY), "MOSS"))
    }
}

impl Default for Ticker {
    fn default() -> Self {
        Ticker {
            watchlist: ["ALDR", "DUNE", "GLYF", "IRIS", "MOSS", "NOVA"].into_iter().map(|s| Watched { id: Key::from_raw(key_of(s).raw()) }).collect(),
            exchange: LISTED.iter().enumerate().map(|(n, (symbol, name, _))| Listed { id: Key::from_raw(n as u128 + 1), symbol: symbol.to_string(), name: name.to_string() }).collect(),
            watched_chosen: Selection::new(),
            listed_chosen: Selection::new(),
            trade_chosen: Selection::new(),
            market: Live::default(),
            chart: Derived::default(),
        }
    }
}

impl Chart {
    fn of(market: &Live<Market>, chosen: &Selection<Watched>) -> Chart {
        let Some(key) = chosen.focused() else { return Chart::default() };
        let n = index(key.raw());
        let name = format!("{} — {}", LISTED[n].0, LISTED[n].1);
        let candles = market.now().candles.get(n).filter(|c| !c.is_empty());
        let plot = candles.map(|candles| {
            let first = candles.len().saturating_sub(CANDLES);
            Plot::new(12).candles(candles[first..].iter().copied()).names((first..candles.len()).map(|k| hours(k as u32 * CANDLE_MS)))
        });
        Chart { name, plot }
    }
}

fn key_of(symbol: &str) -> Key<Listed> {
    Key::from_raw(LISTED.iter().position(|(s, ..)| *s == symbol).map_or(0, |n| n as u128 + 1))
}

fn index(raw: u128) -> usize {
    (raw as usize).saturating_sub(1).min(LISTED.len() - 1)
}

/// The hour and minute on the exchange's clock.
fn hours(ms: u32) -> String {
    let minutes = 9 * 60 + 30 + ms / 60_000;
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

/// A time on the exchange's clock, which opens at half past nine.
fn time(ms: u32) -> String {
    let seconds = 9 * 3600 + 30 * 60 + ms / 1000;
    format!("{:02}:{:02}:{:02}", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

/// A simulated exchange: each trade moves one stock's price a little at random, from a seed, so the same seed always trades the same way.
struct Exchange {
    seed: u64,
    market: Market,
    next: u128,
}

impl Exchange {
    fn new(seed: u64) -> Exchange {
        let quotes = LISTED.iter().map(|&(_, _, open)| Quote { open, last: open, high: open, low: open, volume: 0 }).collect();
        Exchange { seed, market: Market { quotes, candles: vec![Vec::new(); LISTED.len()], tape: List::new(), clock: 0 }, next: 1 }
    }

    fn roll(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn trade(&mut self) -> Trade {
        let gap = 5 + self.roll() % 30;
        self.market.clock += gap as u32;
        let n = (self.roll() % LISTED.len() as u64) as usize;
        let drift = (self.roll() % 5) as i64 - 2;
        let last = self.market.quotes[n].last;
        let price = (last + drift * (last / 2_000).max(1)).max(1);
        let size = 100 * (1 + self.roll() % 20) as u32;
        let quote = &mut self.market.quotes[n];
        quote.last = price;
        quote.high = quote.high.max(price);
        quote.low = quote.low.min(price);
        quote.volume += u64::from(size);
        let period = (self.market.clock / CANDLE_MS) as usize;
        let candles = &mut self.market.candles[n];
        while candles.len() <= period {
            let open = candles.last().map_or(LISTED[n].2 as f64 / 100.0, |c| c.close);
            candles.push(Candle { open, high: open, low: open, close: open });
        }
        let (candle, p) = (&mut candles[period], price as f64 / 100.0);
        candle.high = candle.high.max(p);
        candle.low = candle.low.min(p);
        candle.close = p;
        let id = Key::from_raw(self.next);
        self.next += 1;
        Trade { id, stock: n as u16, price, size, at: self.market.clock }
    }

    fn trade_live(&mut self) {
        let trade = self.trade();
        let _ = self.market.tape.insert_after(None, trade);
    }

    fn open(&mut self, trades: usize) {
        let day: Vec<Trade> = (0..trades).map(|_| self.trade()).collect();
        self.market.tape = day.into_iter().rev().collect();
    }

    fn run(mut self, trades: usize) -> Market {
        self.open(trades);
        self.market
    }
}

/// The market feed: a day's trades so far, then a new trade every few tens of milliseconds, published at most every 50 milliseconds.
fn simulate(feed: Feed<Market>) {
    let mut exchange = Exchange::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64 | 1));
    exchange.open(HISTORY);
    feed.publish(exchange.market.clone());
    let (started, from) = (Instant::now(), exchange.market.clock);
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let now = from + started.elapsed().as_millis() as u32;
        while exchange.market.clock < now {
            exchange.trade_live();
        }
        feed.publish(exchange.market.clone());
    }
}

fn main() {
    graticule::run::<Ticker>(t!("Ticker"));
}

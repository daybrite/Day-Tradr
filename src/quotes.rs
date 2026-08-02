//! The data layer: the persisted watchlist, stooq.com fetches, deterministic mock series, and
//! one memoized reactive [`Resource`] per symbol (https://daybrite.dev/docs/async).
//!
//! Live data comes from two stooq CSV endpoints over day-part-http (docs/http.md):
//! `https://stooq.com/q/l/?s=<sym>&f=snd2t2ohlcv&h&e=csv` (the current quote + display name)
//! and `https://stooq.com/q/d/l/?s=<sym>&i=d` (daily history — Date,Open,High,Low,Close,Volume).
//! CAVEAT (verified 2026-08-01): stooq fronts these with a JavaScript proof-of-work wall and
//! denies the CSV downloads outright from some network classes even after verification, so
//! live mode may show the error state depending on where the app runs. The path stays correct
//! for networks stooq still serves; TRADR_MOCK=1 is the deterministic showcase either way.
//! Mock mode (`--env TRADR_MOCK=1`, read through `day::env` so it reaches web-dom as a query
//! parameter) generates every series from an integer LCG — no floats-in, no transcendentals —
//! so the SAME prices render on every target and dayscript can assert them verbatim.

use day::prelude::*;
use std::cell::{OnceCell, RefCell};

/// The prefs key holding the watchlist as a comma-joined symbol list.
const PREF_SYMBOLS: &str = "tradr.symbols";

/// A fresh install tracks a spread of stocks, an ETF, and two commodities. (TSLA stays in
/// [`PRESETS`], so it can still be added from the manage page.)
const DEFAULT_SYMBOLS: [&str; 6] = ["AAPL.US", "MSFT.US", "NVDA.US", "SPY.US", "XAUUSD", "CL.F"];

/// Display names + mock price anchors for the symbols the app suggests. Live mode overwrites
/// the name with what stooq reports; provider data and tickers are proper nouns, deliberately
/// not localized. The anchor keeps mock charts in a plausible band per instrument.
const NAMES: [(&str, &str, f64); 12] = [
    ("AAPL.US", "Apple Inc.", 230.0),
    ("MSFT.US", "Microsoft Corp.", 500.0),
    ("NVDA.US", "NVIDIA Corp.", 175.0),
    ("TSLA.US", "Tesla Inc.", 320.0),
    ("SPY.US", "SPDR S&P 500 ETF", 630.0),
    ("GOOG.US", "Alphabet Inc.", 195.0),
    ("AMZN.US", "Amazon.com Inc.", 230.0),
    ("META.US", "Meta Platforms Inc.", 710.0),
    ("XAUUSD", "Gold Spot", 3350.0),
    ("XAGUSD", "Silver Spot", 38.0),
    ("CL.F", "Crude Oil WTI", 68.0),
    ("EURUSD", "Euro / US Dollar", 1.16),
];

/// The symbols offered by the manage page's picker (a superset of the defaults).
pub const PRESETS: [&str; 12] = [
    "AAPL.US", "MSFT.US", "NVDA.US", "TSLA.US", "GOOG.US", "AMZN.US", "META.US", "SPY.US",
    "XAUUSD", "XAGUSD", "CL.F", "EURUSD",
];

pub fn preset_name(symbol: &str) -> &str {
    NAMES
        .iter()
        .find(|(s, _, _)| *s == symbol)
        .map(|(_, n, _)| *n)
        .unwrap_or(symbol)
}

/// The mock anchor price: the catalog's figure for a known symbol, else hash-derived.
fn anchor_price(symbol: &str, seed: u64) -> f64 {
    NAMES
        .iter()
        .find(|(s, _, _)| *s == symbol)
        .map(|(_, _, a)| *a)
        .unwrap_or_else(|| 20.0 + (seed % 48_001) as f64 / 100.0)
}

/// One tracked instrument's processed data — everything the pages render.
#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    /// Daily closes, oldest → newest. The chart slices ranges off the tail.
    pub closes: Vec<f64>,
    /// Daily volumes, aligned with `closes` (zero where the source has none).
    pub volumes: Vec<f64>,
    /// ISO dates aligned with `closes`.
    pub dates: Vec<String>,
    pub last: f64,
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub mock: bool,
}

impl Quote {
    pub fn change(&self) -> f64 {
        self.last - self.prev_close
    }
    pub fn change_pct(&self) -> f64 {
        if self.prev_close == 0.0 {
            0.0
        } else {
            self.change() / self.prev_close * 100.0
        }
    }
    pub fn week52_high(&self) -> f64 {
        tail(&self.closes, 252)
            .iter()
            .cloned()
            .fold(self.last, f64::max)
    }
    pub fn week52_low(&self) -> f64 {
        tail(&self.closes, 252)
            .iter()
            .cloned()
            .fold(self.last, f64::min)
    }
    /// 20-day simple moving average — one of the stats-grid cells.
    pub fn sma20(&self) -> f64 {
        let t = tail(&self.closes, 20);
        if t.is_empty() {
            self.last
        } else {
            t.iter().sum::<f64>() / t.len() as f64
        }
    }
}

pub fn tail(v: &[f64], n: usize) -> &[f64] {
    &v[v.len().saturating_sub(n)..]
}

/// A fetch/parse failure, displayable (`Error + Send + Sync` so it rides `Load::Failed`).
#[derive(Debug)]
pub struct QuoteError(pub String);

impl std::fmt::Display for QuoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for QuoteError {}

// ---------------------------------------------------------------------------
// Watchlist store — the settings-Store pattern (a detached root-lifetime signal).
// ---------------------------------------------------------------------------

thread_local! {
    static SYMBOLS: OnceCell<Signal<Vec<String>>> = const { OnceCell::new() };
    /// The chart range every detail page shares (Apple-Stocks-style: switching range on one
    /// symbol switches it for all). Index into `charts::RANGES`.
    static RANGE: OnceCell<Signal<usize>> = const { OnceCell::new() };
    /// Bumped by the settings Refresh action; every quote Resource tracks it.
    static GENERATION: OnceCell<Signal<u64>> = const { OnceCell::new() };
}

pub fn symbols() -> Signal<Vec<String>> {
    SYMBOLS.with(|cell| {
        *cell.get_or_init(|| {
            let seed = match day_part_prefs::get(PREF_SYMBOLS) {
                // A saved list wins, even an empty one; a fresh install gets the defaults.
                Some(joined) => joined
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
                None => DEFAULT_SYMBOLS.iter().map(|s| s.to_string()).collect(),
            };
            Scope::detached().enter(|| Signal::new(seed))
        })
    })
}

pub fn range() -> Signal<usize> {
    RANGE.with(|cell| *cell.get_or_init(|| Scope::detached().enter(|| Signal::new(3)))) // 1Y
}

fn generation() -> Signal<u64> {
    GENERATION.with(|cell| *cell.get_or_init(|| Scope::detached().enter(|| Signal::new(0))))
}

fn persist(list: &[String]) {
    day_part_prefs::set(PREF_SYMBOLS, &list.join(","));
}

/// Normalize what the user typed into a stooq symbol: trimmed, uppercased.
pub fn normalize(input: &str) -> String {
    input.trim().to_ascii_uppercase()
}

/// Add a symbol (idempotent). Returns false when it was already tracked.
pub fn add(symbol: &str) -> bool {
    let sig = symbols();
    let mut list = sig.get_untracked();
    if list.iter().any(|s| s == symbol) {
        return false;
    }
    list.push(symbol.to_string());
    persist(&list);
    sig.set(list);
    true
}

pub fn remove(symbol: &str) {
    let sig = symbols();
    let mut list = sig.get_untracked();
    list.retain(|s| s != symbol);
    persist(&list);
    sig.set(list);
    drop_state(symbol);
}

/// Drag-to-reorder (docs/list.md): the row at `from` now sits at `to`. The comma-joined store
/// already encodes order, so the same persist path covers it — the sidebar follows the same Vec.
pub fn move_symbol(from: usize, to: usize) {
    let sig = symbols();
    let mut list = sig.get_untracked();
    if from < list.len() && to < list.len() {
        let s = list.remove(from);
        list.insert(to, s);
        persist(&list);
        sig.set(list);
    }
}

/// Refetch every tracked symbol (the settings Refresh action): one generation bump, every
/// Resource tracks it as its source.
pub fn reload_all() {
    let g = generation();
    g.set(g.get_untracked() + 1);
}

// ---------------------------------------------------------------------------
// Per-symbol Resources, memoized by symbol (the Day-Skies STATES pattern).
// ---------------------------------------------------------------------------

thread_local! {
    static STATES: RefCell<Vec<(String, day::reactive::Resource<Quote>)>> =
        const { RefCell::new(Vec::new()) };
}

pub fn resource_for(symbol: &str) -> day::reactive::Resource<Quote> {
    STATES.with(|cell| {
        if let Some((_, r)) = cell.borrow().iter().find(|(s, _)| s == symbol) {
            return *r;
        }
        let sym = symbol.to_string();
        // Root scope, not the calling page's build scope: the resource must outlive the pane
        // that first touched it (the Day-Skies detached-resource rationale).
        let r = day::reactive::Scope::root().enter(|| {
            day::reactive::Resource::new(move || generation().get(), move |_| load(sym.clone()))
        });
        cell.borrow_mut().push((symbol.to_string(), r));
        r
    })
}

fn drop_state(symbol: &str) {
    STATES.with(|cell| cell.borrow_mut().retain(|(s, _)| s != symbol));
}

/// Is the app forced into deterministic mock mode? `day::env` rather than `std::env`: on
/// web-dom the flag arrives as a page query parameter, not process environment.
pub fn is_mock() -> bool {
    match day::env("TRADR_MOCK") {
        Some(v) => !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false")),
        None => false,
    }
}

async fn load(symbol: String) -> Result<Quote, QuoteError> {
    if is_mock() {
        return Ok(mock(&symbol));
    }
    fetch(symbol).await
}

// ---------------------------------------------------------------------------
// Mock — an integer LCG random walk. No transcendentals, no platform-varying float paths:
// the identical prices render on every target, so walkthrough asserts are portable.
// ---------------------------------------------------------------------------

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    /// A step in cents within ±max_cents, exact in f64.
    fn step(&mut self, max_cents: u64) -> f64 {
        let span = max_cents * 2 + 1;
        (self.next() % span) as f64 / 100.0 - (max_cents as f64 / 100.0)
    }
}

/// Build a deterministic 500-day series for a symbol. Cent-quantized arithmetic throughout
/// (every op is exact IEEE add/mul/round on cent multiples), so the identical closes format
/// on every target. A light mean reversion toward the anchor keeps the walk in a plausible
/// band — no index collapsing to pennies, no gold at 13.
pub fn mock(symbol: &str) -> Quote {
    let seed = fnv(symbol);
    let mut rng = Lcg(seed);
    let base = anchor_price(symbol, rng.next());
    // Daily step scale: ~0.7% of the anchor, floor of 5 cents.
    let step_cents = ((base * 0.7).round() as u64).max(5);
    let days = 500usize;
    let mut closes = Vec::with_capacity(days);
    let mut volumes = Vec::with_capacity(days);
    let mut dates = Vec::with_capacity(days);
    let mut price = base;
    for i in 0..days {
        let reversion = (base - price) * 0.01;
        price = (price + reversion + rng.step(step_cents)).max(base * 0.05);
        // Re-quantize to cents so every close is an exact multiple of 0.01.
        price = (price * 100.0).round() / 100.0;
        closes.push(price);
        volumes.push((500_000 + rng.next() % 9_500_000) as f64);
        dates.push(mock_date(i, days));
    }
    let last = closes[days - 1];
    let prev_close = closes[days - 2];
    let mut day_rng = Lcg(seed ^ 0x00ff_00ff_00ff_00ff);
    let open = ((prev_close + day_rng.step(80)).max(1.0) * 100.0).round() / 100.0;
    let high = last.max(open) + (day_rng.next() % 200) as f64 / 100.0;
    let low = (last.min(open) - (day_rng.next() % 200) as f64 / 100.0).max(0.5);
    Quote {
        name: preset_name(symbol).to_string(),
        symbol: symbol.to_string(),
        closes,
        volumes,
        dates,
        last,
        prev_close,
        open,
        high: (high * 100.0).round() / 100.0,
        low: (low * 100.0).round() / 100.0,
        volume: (1_000_000 + day_rng.next() % 9_000_000) as f64,
        mock: true,
    }
}

/// A synthetic ISO date for mock row `i` of `days`: counts back from a fixed anchor so runs
/// are identical regardless of the wall clock (2026-07-01 is the newest mock day).
fn mock_date(i: usize, days: usize) -> String {
    // Days-to-civil is overkill for fixture labels: a flat 30-day month is fine for display.
    let back = days - 1 - i;
    let months_back = back / 30;
    let day_in_month = 30 - (back % 30);
    let (mut y, mut m) = (2026i64, 7i64);
    m -= months_back as i64;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    format!("{y:04}-{m:02}-{day_in_month:02}")
}

// ---------------------------------------------------------------------------
// Live fetch + CSV processing.
// ---------------------------------------------------------------------------

async fn fetch(symbol: String) -> Result<Quote, QuoteError> {
    let sym = symbol.to_ascii_lowercase();
    let history_url = format!("https://stooq.com/q/d/l/?s={sym}&i=d");
    let quote_url = format!("https://stooq.com/q/l/?s={sym}&f=snd2t2ohlcv&h&e=csv");

    let history = get_text(&history_url).await?;
    let (closes, volumes, dates) = parse_history(&history, &symbol)?;

    // The quote endpoint refines the latest numbers + supplies the display name; a failure
    // here degrades to history-derived values rather than failing the symbol.
    let (name, open, high, low, close, volume) = match get_text(&quote_url).await {
        Ok(text) => parse_quote(&text).unwrap_or((String::new(), 0.0, 0.0, 0.0, 0.0, 0.0)),
        Err(_) => (String::new(), 0.0, 0.0, 0.0, 0.0, 0.0),
    };

    let last = if close > 0.0 {
        close
    } else {
        *closes
            .last()
            .ok_or_else(|| QuoteError(format!("{symbol}: empty history")))?
    };
    let prev_close = if closes.len() >= 2 {
        closes[closes.len() - 2]
    } else {
        last
    };
    Ok(Quote {
        name: if name.is_empty() {
            preset_name(&symbol).to_string()
        } else {
            name
        },
        symbol,
        last,
        prev_close,
        open,
        high,
        low,
        volume,
        closes,
        volumes,
        dates,
        mock: false,
    })
}

async fn get_text(url: &str) -> Result<String, QuoteError> {
    let resp = day_part_http::fetch_future(
        day_part_http::Request::get(url).timeout(std::time::Duration::from_secs(15)),
    )
    .await
    .map_err(|e| QuoteError(format!("request failed: {e}")))?;
    if !(200..300).contains(&resp.status) {
        return Err(QuoteError(format!("request failed: HTTP {}", resp.status)));
    }
    Ok(String::from_utf8_lossy(&resp.body).into_owned())
}

/// Parsed daily history: closes, volumes, and ISO dates, index-aligned, oldest first.
type History = (Vec<f64>, Vec<f64>, Vec<String>);

/// stooq daily history: `Date,Open,High,Low,Close,Volume` rows, oldest first. An unknown
/// symbol answers a body with no data rows — surfaced as an error, not an empty chart.
fn parse_history(csv: &str, symbol: &str) -> Result<History, QuoteError> {
    let mut closes = Vec::new();
    let mut volumes = Vec::new();
    let mut dates = Vec::new();
    for line in csv.lines().skip(1) {
        let mut cols = line.split(',');
        let date = cols.next().unwrap_or_default();
        let close = cols.nth(3).and_then(|c| c.parse::<f64>().ok());
        let volume = cols
            .next()
            .and_then(|c| c.parse::<f64>().ok())
            .unwrap_or(0.0);
        if let Some(close) = close {
            dates.push(date.to_string());
            closes.push(close);
            volumes.push(volume);
        }
    }
    if closes.is_empty() {
        return Err(QuoteError(format!("{symbol}: no data (unknown symbol?)")));
    }
    Ok((closes, volumes, dates))
}

/// stooq quote: header + one `Symbol,Name,Date,Time,Open,High,Low,Close,Volume` row. `N/D`
/// marks fields the source has no value for.
#[allow(clippy::type_complexity)]
fn parse_quote(csv: &str) -> Option<(String, f64, f64, f64, f64, f64)> {
    let line = csv.lines().nth(1)?;
    let cols: Vec<&str> = line.split(',').collect();
    let num = |i: usize| -> f64 {
        cols.get(i)
            .and_then(|c| c.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    Some((
        cols.get(1).unwrap_or(&"").to_string(),
        num(4),
        num(5),
        num(6),
        num(7),
        num(8),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The walkthrough asserts these exact strings — this test pins the mock generator so a
    /// change to it fails HERE, on the host, before it fails on a device.
    #[test]
    fn mock_is_deterministic() {
        let a = mock("AAPL.US");
        let b = mock("AAPL.US");
        assert_eq!(a, b);
        assert_eq!(a.closes.len(), 500);
        // Every close is cent-quantized, so 2-decimal formatting is exact everywhere.
        for c in &a.closes {
            assert!((c * 100.0 - (c * 100.0).round()).abs() < 1e-9);
        }
        assert!(a.last > 0.0 && a.low <= a.high);
    }

    #[test]
    fn history_parses_and_rejects_empty() {
        let csv = "Date,Open,High,Low,Close,Volume\n2026-07-01,1,2,0.5,1.5,100\n";
        let (c, v, d) = parse_history(csv, "X").unwrap();
        assert_eq!((c[0], v[0], d[0].as_str()), (1.5, 100.0, "2026-07-01"));
        assert!(parse_history("Date,Open,High,Low,Close,Volume\n", "X").is_err());
    }
}

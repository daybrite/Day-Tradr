//! The data layer: the persisted watchlist, Yahoo Finance fetches, deterministic mock series,
//! and one memoized reactive [`Resource`] per symbol (https://daybrite.dev/docs/async).
//!
//! Live data comes from ONE unofficial Yahoo Finance endpoint over day-part-http (docs/http.md):
//! `https://query1.finance.yahoo.com/v8/finance/chart/<sym>?range=2y&interval=1d`. The response
//! carries the daily history AND the current quote (`meta`) together, so a symbol costs a single
//! request rather than a history call plus a quote call.
//!
//! WHY NOT the CSV download endpoint (`/v7/finance/download/<sym>`): Yahoo closed it to anonymous
//! callers in 2024 — it answers `401 {"code":"unauthorized"}` without a session cookie and a
//! matching `crumb` token (verified again 2026-08-11 on `query1` and `query2`). Reaching it means
//! scraping a cookie + crumb before every fetch, which is both fragile and exactly the flakiness
//! this app moved away from. The `v8/chart` JSON endpoint needs no cookie, no crumb, and no API
//! key; it does require a `User-Agent` (an absent one draws `429`), so [`get_text`] sends the
//! app's own.
//!
//! Symbols are Yahoo tickers as typed on finance.yahoo.com: `AAPL`, `SPY`, futures as `CL=F` /
//! `GC=F`, FX as `EURUSD=X`. Watchlists saved by an older build used the previous provider's
//! spelling, so [`migrate_symbol`] rewrites those on load.
//!
//! Mock mode (`--env TRADR_MOCK=1`, read through `day::env` so it reaches web-dom as a query
//! parameter) generates every series from an integer LCG — no floats-in, no transcendentals —
//! so the SAME prices render on every target and dayscript can assert them verbatim.

use day::prelude::*;
use std::cell::{OnceCell, RefCell};

/// The prefs key holding the watchlist as a comma-joined symbol list.
const PREF_SYMBOLS: &str = "tradr.symbols";
/// View preferences, persisted beside the watchlist so the app opens the way it was left.
const PREF_SORT: &str = "tradr.sort";
const PREF_CHIP: &str = "tradr.chip";
const PREF_OVERLAY: &str = "tradr.overlay";
/// The HTTP proxy template quote fetches go through; empty means fetch Yahoo directly.
const PREF_PROXY: &str = "tradr.proxy";

/// What the web build uses unless the user says otherwise.
///
/// A browser will not let a page fetch `query1.finance.yahoo.com` — Yahoo sends no
/// `Access-Control-Allow-Origin`, so every quote request fails CORS before it leaves the tab.
/// A relaying proxy answers with `Access-Control-Allow-Origin: *` and fetches Yahoo server-side,
/// where the rule does not apply. Native builds talk to Yahoo directly and default to empty:
/// there is no CORS on a socket, and a proxy would only add a hop and a stranger.
pub const DEFAULT_WEB_PROXY: &str = "https://api.allorigins.win/raw?url=%u";

/// The Daybrite relay, for anyone who would rather not depend on a public one.
///
/// The public relays are open proxies serving whoever finds them, and they behave like it: a
/// six-symbol load against `api.allorigins.win` completed one request the first time it was
/// measured, which is why [`get_text_resilient`] gates and retries. `proxy.daybrite.dev` is a
/// small Cloudflare Worker that reaches an allowlist of endpoints and nothing else — its source
/// and setup steps live in `proxy/` alongside this app. It takes the target as a PATH under a
/// logical site name rather than as an encoded parameter, which is what `%p` is for.
pub const DAYBRITE_WEB_PROXY: &str = "https://proxy.daybrite.dev/sites/finance/%p";

/// A fresh install tracks a spread of stocks, an ETF, and two commodities. (TSLA stays in
/// [`PRESETS`], so it can still be added from the manage page.)
const DEFAULT_SYMBOLS: [&str; 6] = ["AAPL", "MSFT", "NVDA", "SPY", "GC=F", "CL=F"];

/// Display names + mock price anchors for the symbols the app suggests. Live mode overwrites
/// the name with what Yahoo reports; provider data and tickers are proper nouns, deliberately
/// not localized. The anchor keeps mock charts in a plausible band per instrument.
const NAMES: [(&str, &str, f64); 12] = [
    ("AAPL", "Apple Inc.", 230.0),
    ("MSFT", "Microsoft Corp.", 500.0),
    ("NVDA", "NVIDIA Corp.", 175.0),
    ("TSLA", "Tesla Inc.", 320.0),
    ("SPY", "SPDR S&P 500 ETF", 630.0),
    ("GOOG", "Alphabet Inc.", 195.0),
    ("AMZN", "Amazon.com Inc.", 230.0),
    ("META", "Meta Platforms Inc.", 710.0),
    ("GC=F", "Gold Futures", 3350.0),
    ("SI=F", "Silver Futures", 38.0),
    ("CL=F", "Crude Oil WTI", 68.0),
    ("EURUSD=X", "Euro / US Dollar", 1.16),
];

/// The symbols offered by the manage page's picker (a superset of the defaults).
pub const PRESETS: [&str; 12] = [
    "AAPL", "MSFT", "NVDA", "TSLA", "GOOG", "AMZN", "META", "SPY", "GC=F", "SI=F", "CL=F",
    "EURUSD=X",
];

/// Rewrite a watchlist entry saved by a build that used the previous provider's tickers. Yahoo
/// spells US equities bare (`AAPL`, not `AAPL.US`) and quotes metals and oil as futures
/// contracts rather than spot, so an upgrade would otherwise show every saved row as an unknown
/// symbol. Anything already in Yahoo's spelling passes through untouched.
fn migrate_symbol(symbol: &str) -> String {
    const LEGACY: [(&str, &str); 4] = [
        ("XAUUSD", "GC=F"),
        ("XAGUSD", "SI=F"),
        ("CL.F", "CL=F"),
        ("EURUSD", "EURUSD=X"),
    ];
    if let Some((_, yahoo)) = LEGACY.iter().find(|(old, _)| *old == symbol) {
        return (*yahoo).to_string();
    }
    match symbol.strip_suffix(".US") {
        Some(base) => base.to_string(),
        None => symbol.to_string(),
    }
}

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

    /// The 50-day average, beside [`Self::sma20`] in the chart's overlay legend.
    pub fn sma50(&self) -> f64 {
        let t = tail(&self.closes, 50);
        if t.is_empty() {
            self.last
        } else {
            t.iter().sum::<f64>() / t.len() as f64
        }
    }

    /// Where `last` sits between `lo` and `hi`, as 0…1. Both range bars read this; a degenerate
    /// range (a flat series, or a symbol with one session) centres the marker rather than
    /// dividing by zero.
    pub fn position_in(&self, lo: f64, hi: f64) -> f64 {
        if hi - lo <= f64::EPSILON {
            0.5
        } else {
            ((self.last - lo) / (hi - lo)).clamp(0.0, 1.0)
        }
    }
}

/// A rolling `n`-day simple moving average over `closes`, index-aligned with it: `None` until
/// there are `n` samples to average, so the chart starts each overlay where it becomes real
/// rather than drawing a misleading ramp from the first bar.
pub fn sma_series(closes: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = Vec::with_capacity(closes.len());
    let mut sum = 0.0;
    for (i, c) in closes.iter().enumerate() {
        sum += c;
        if i >= n {
            sum -= closes[i - n];
        }
        out.push(if i + 1 >= n {
            Some(sum / n as f64)
        } else {
            None
        });
    }
    out
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
            let seed: Vec<String> = match day_part_prefs::get(PREF_SYMBOLS) {
                // A saved list wins, even an empty one; a fresh install gets the defaults.
                // Entries saved under the previous provider's tickers are rewritten on the way
                // in, so an upgrade keeps its watchlist instead of showing unknown symbols.
                Some(joined) => joined
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(migrate_symbol)
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

/// How the watchlist is ordered. `Manual` is the drag-reordered list the user owns; the other
/// two are views over it, so switching back to `Manual` restores their arrangement untouched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Manual,
    Name,
    /// Biggest gainer first — the "what moved today" ordering.
    Change,
}

impl Sort {
    pub const ALL: [Sort; 3] = [Sort::Manual, Sort::Name, Sort::Change];
    fn key(self) -> &'static str {
        match self {
            Sort::Manual => "manual",
            Sort::Name => "name",
            Sort::Change => "change",
        }
    }
    fn from_key(s: &str) -> Sort {
        Self::ALL
            .into_iter()
            .find(|v| v.key() == s)
            .unwrap_or(Sort::Manual)
    }
}

/// What the change chip shows. Tapping any chip cycles every chip at once (the Apple Stocks
/// behaviour: the column is one control, not a per-row setting).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipMode {
    /// `+1.24 (0.62%)` — absolute move and percentage together.
    Both,
    /// `+0.62%` — percentage alone, the densest reading for scanning a long list.
    Percent,
    /// `+1.24` — the move in the instrument's own units.
    Absolute,
}

impl ChipMode {
    fn key(self) -> &'static str {
        match self {
            ChipMode::Both => "both",
            ChipMode::Percent => "percent",
            ChipMode::Absolute => "absolute",
        }
    }
    /// The order tapping walks: the fullest reading first, then the two narrow ones.
    pub fn next(self) -> ChipMode {
        match self {
            ChipMode::Both => ChipMode::Percent,
            ChipMode::Percent => ChipMode::Absolute,
            ChipMode::Absolute => ChipMode::Both,
        }
    }
    fn from_key(s: &str) -> ChipMode {
        match s {
            "percent" => ChipMode::Percent,
            "absolute" => ChipMode::Absolute,
            _ => ChipMode::Both,
        }
    }
}

thread_local! {
    static PROXY: OnceCell<Signal<String>> = const { OnceCell::new() };
    static SORT: OnceCell<Signal<Sort>> = const { OnceCell::new() };
    static CHIP: OnceCell<Signal<ChipMode>> = const { OnceCell::new() };
    static OVERLAY: OnceCell<Signal<bool>> = const { OnceCell::new() };
}

/// The proxy template, persisted. Seeded from prefs; a fresh install gets
/// [`DEFAULT_WEB_PROXY`] on the web build and nothing anywhere else. A SAVED empty string is
/// honoured as "direct" — `is_some` distinguishes it from never having been set, so a web user
/// who deliberately clears the field does not get the default handed back on next launch.
pub fn proxy() -> Signal<String> {
    PROXY.with(|cell| {
        *cell.get_or_init(|| {
            let seed = day_part_prefs::get(PREF_PROXY).unwrap_or_else(|| {
                if cfg!(feature = "dom") {
                    DEFAULT_WEB_PROXY.to_string()
                } else {
                    String::new()
                }
            });
            Scope::detached().enter(|| Signal::new(seed))
        })
    })
}

pub fn set_proxy(template: &str) {
    day_part_prefs::set(PREF_PROXY, template);
    proxy().set(template.to_string());
}

/// Route `url` through the proxy `template`.
///
/// `%u` is replaced with the PERCENT-ENCODED url, because the templates that use a placeholder
/// put it in a query parameter and the target carries its own query string: substituted raw,
/// `…?url=https://…/chart/AAPL?range=2y&interval=1d` hands `interval` to the PROXY instead of to
/// Yahoo, and the request 500s (measured against api.allorigins.win, which answers 200 with the
/// encoded form and 500 with the raw one).
///
/// `%p` is replaced with the target's PATH AND QUERY, minus the leading slash. That is the shape
/// an allowlisting relay uses, where the host is already decided by the template and only the
/// path travels: `https://proxy.daybrite.dev/sites/finance/%p` becomes
/// `https://proxy.daybrite.dev/sites/finance/v8/finance/chart/AAPL?range=2y&interval=1d`. Nothing
/// is re-encoded here — the path is already escaped, and encoding it again would send the relay a
/// literal `%3D` to look up.
///
/// A template with no placeholder is treated as a PREFIX and the raw url is appended — the shape
/// the cors-anywhere family uses (`https://proxy.example/https://target`). An empty template
/// fetches directly.
pub fn proxied(template: &str, url: &str) -> String {
    let template = template.trim();
    if template.is_empty() {
        url.to_string()
    } else if template.contains("%p") {
        template.replace("%p", path_and_query(url))
    } else if template.contains("%u") {
        template.replace("%u", &percent_encode(url))
    } else {
        format!("{template}{url}")
    }
}

/// The path and query of an absolute url, without the leading slash, so a template can end in
/// the `/` that separates it (`…/sites/finance/` + `v8/finance/chart/AAPL`).
///
/// A url with no path at all yields an empty string rather than borrowing past the end.
fn path_and_query(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    match rest.find('/') {
        Some(slash) => &rest[slash + 1..],
        None => "",
    }
}

pub fn sort() -> Signal<Sort> {
    SORT.with(|cell| {
        *cell.get_or_init(|| {
            let seed = day_part_prefs::get(PREF_SORT)
                .map(|s| Sort::from_key(&s))
                .unwrap_or(Sort::Manual);
            Scope::detached().enter(|| Signal::new(seed))
        })
    })
}

pub fn set_sort(v: Sort) {
    day_part_prefs::set(PREF_SORT, v.key());
    sort().set(v);
}

pub fn chip_mode() -> Signal<ChipMode> {
    CHIP.with(|cell| {
        *cell.get_or_init(|| {
            let seed = day_part_prefs::get(PREF_CHIP)
                .map(|s| ChipMode::from_key(&s))
                .unwrap_or(ChipMode::Both);
            Scope::detached().enter(|| Signal::new(seed))
        })
    })
}

/// Advance every chip one step and remember it.
pub fn cycle_chip_mode() {
    let next = chip_mode().get_untracked().next();
    day_part_prefs::set(PREF_CHIP, next.key());
    chip_mode().set(next);
}

/// Whether the detail chart draws its moving-average overlays.
pub fn overlay() -> Signal<bool> {
    OVERLAY.with(|cell| {
        *cell.get_or_init(|| {
            let seed = day_part_prefs::get(PREF_OVERLAY).is_none_or(|s| s != "0");
            Scope::detached().enter(|| Signal::new(seed))
        })
    })
}

/// Write the overlay preference to disk WITHOUT touching the signal.
///
/// The toggle is bound two-way to [`overlay`], so the signal already carries the user's choice
/// by the time anything wants to persist it. A persist function that also `set` the signal
/// would close a loop with the effect that watches it — the effect reads, writes, and re-reads
/// until the reactive runtime trips its cycle guard and the main thread stops responding.
pub fn persist_overlay(on: bool) {
    day_part_prefs::set(PREF_OVERLAY, if on { "1" } else { "0" });
}

/// The watchlist in display order: the user's own arrangement, or a view sorted by name or by
/// today's move. Symbols still loading sort last under `Change` rather than jumping around as
/// each fetch lands.
pub fn sorted_symbols(list: Vec<String>, order: Sort) -> Vec<String> {
    let mut out = list;
    match order {
        Sort::Manual => {}
        Sort::Name => out.sort(),
        Sort::Change => out.sort_by(|a, b| {
            let pct = |s: &String| {
                resource_for(s)
                    .signal()
                    .with(|l| l.ready().map(|q| q.change_pct()))
            };
            match (pct(a), pct(b)) {
                (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.cmp(b),
            }
        }),
    }
    out
}

/// Today's breadth across the watchlist: how many symbols are up, how many down, and the
/// biggest mover in each direction. `None` until at least one quote has loaded.
pub struct Breadth {
    pub up: usize,
    pub down: usize,
    pub best: Option<(String, f64)>,
    pub worst: Option<(String, f64)>,
}

pub fn breadth(list: &[String]) -> Option<Breadth> {
    let mut b = Breadth {
        up: 0,
        down: 0,
        best: None,
        worst: None,
    };
    let mut any = false;
    for s in list {
        let Some(pct) = resource_for(s)
            .signal()
            .with(|l| l.ready().map(|q| q.change_pct()))
        else {
            continue;
        };
        any = true;
        if pct >= 0.0 {
            b.up += 1;
        } else {
            b.down += 1;
        }
        if b.best.as_ref().is_none_or(|(_, v)| pct > *v) {
            b.best = Some((s.clone(), pct));
        }
        if b.worst.as_ref().is_none_or(|(_, v)| pct < *v) {
            b.worst = Some((s.clone(), pct));
        }
    }
    any.then_some(b)
}

fn generation() -> Signal<u64> {
    GENERATION.with(|cell| *cell.get_or_init(|| Scope::detached().enter(|| Signal::new(0))))
}

fn persist(list: &[String]) {
    day_part_prefs::set(PREF_SYMBOLS, &list.join(","));
}

/// Normalize what the user typed into a Yahoo ticker: trimmed, uppercased.
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

/// Two years of daily bars: enough for the widest range the picker offers (1Y = 252 trading
/// days) plus the 52-week statistics, in one response.
const CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart/";

async fn fetch(symbol: String) -> Result<Quote, QuoteError> {
    // Percent-encode the ticker: futures and FX carry `=` (`CL=F`, `EURUSD=X`), which is legal
    // in a path segment but not worth betting the request on.
    let url = format!(
        "{CHART_URL}{}?range=2y&interval=1d",
        percent_encode(&symbol)
    );
    let template = proxy().get_untracked();
    let route = proxied(&template, &url);
    let body = get_text_resilient(&route, route != url).await?;
    parse_chart(&body, &symbol)
}

/// Encode the characters that appear in Yahoo tickers and mean something in a URL.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// How many quote fetches may be in flight at once while a proxy is in use.
///
/// MEASURED, not guessed: firing the six default symbols at `api.allorigins.win` together
/// returned one body and five gateway timeouts (~19s each) — which is precisely the "could not
/// load" wall a web user meets on first launch. Staggered, the same six mostly land. A public
/// relay is a shared resource with its own rate limiting, so the app queues behind itself
/// rather than stampeding it. Direct fetches talk to Yahoo and need no gate.
const PROXY_MAX_IN_FLIGHT: usize = 2;

thread_local! {
    static IN_FLIGHT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Cooperative semaphore: yield to the main-loop executor until a slot frees up. Day's executor
/// is single-threaded, so a plain counter is enough — there is no other thread to race.
///
/// Yields `None` if `epoch` went stale while waiting, having taken no slot.
async fn gate_acquire(epoch: u64) -> Option<GateSlot> {
    loop {
        if superseded(epoch) {
            return None;
        }
        let got = IN_FLIGHT.with(|c| {
            if c.get() < PROXY_MAX_IN_FLIGHT {
                c.set(c.get() + 1);
                true
            } else {
                false
            }
        });
        if got {
            return Some(GateSlot);
        }
        day::sleep(150).await;
    }
}

/// Has a reload been asked for since this request started?
///
/// Changing the proxy calls [`reload_all`], and without this check the requests aimed at the OLD
/// proxy keep the two gate slots for as long as their retries run — up to a couple of minutes,
/// during which the new setting looks like it did nothing. `Resource` already drops a superseded
/// load, so abandoning one loses nothing.
fn superseded(epoch: u64) -> bool {
    generation().get_untracked() != epoch
}

/// A held slot, given back when it drops.
///
/// Drop rather than a release call at the end of the fetch: superseding a `Resource` load ABORTS
/// it by dropping its future mid-await, so a release written as a statement never runs. Two
/// leaked slots wedge the gate shut and the app stops fetching at all — which is what changing
/// the proxy used to do, since that supersedes every load in flight (measured: zero requests
/// afterwards, forever).
struct GateSlot;

impl Drop for GateSlot {
    fn drop(&mut self) {
        IN_FLIGHT.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

/// Fetch with the retry a public relay needs. Measured failure rate against allorigins was
/// roughly one request in three (500s and 522 gateway timeouts) even sequentially, and a failed
/// symbol shows as a dead row until the next manual refresh — so a couple of quiet retries buy
/// far more than they cost. Only used when a proxy is configured; a direct Yahoo fetch is
/// reliable enough not to need it.
async fn get_text_resilient(url: &str, proxied: bool) -> Result<String, QuoteError> {
    if !proxied {
        return get_text(url, 15).await;
    }
    let epoch = generation().get_untracked();
    let Some(_slot) = gate_acquire(epoch).await else {
        return Err(QuoteError("superseded".to_string()));
    };
    // A relay adds a hop, and a slow-but-successful response took 20s in testing — a 15s
    // timeout would have thrown away a body that was on its way.
    // FIVE attempts, not two or three. Measured success against allorigins is roughly one in
    // two per try — independent of payload size (a 1y request fares no better than 2y) — so a
    // symbol needs several goes before its row stops reading "could not load". They cost
    // nothing while they wait: each symbol retries on its own, and rows fill in as they land.
    let mut last = QuoteError("request failed".to_string());
    for attempt in 0..5u32 {
        match get_text(url, 25).await {
            Ok(body) => return Ok(body),
            Err(e) => {
                last = e;
                // Give up the slot the moment the answer stopped mattering, so the reload that
                // superseded us is not queued behind our remaining retries.
                if superseded(epoch) {
                    return Err(QuoteError("superseded".to_string()));
                }
                if attempt < 4 {
                    day::sleep(600 * (attempt + 1)).await;
                }
            }
        }
    }
    Err(last)
}

async fn get_text(url: &str, timeout_secs: u64) -> Result<String, QuoteError> {
    let resp = day_part_http::fetch_future(
        day_part_http::Request::get(url)
            // Yahoo answers `429 Too Many Requests` to a request with no User-Agent, on the
            // very first call. Identify the app honestly rather than impersonating a browser.
            .header(
                "User-Agent",
                concat!(
                    "Day-Tradr/",
                    env!("CARGO_PKG_VERSION"),
                    " (+https://daybrite.dev)"
                ),
            )
            .timeout(std::time::Duration::from_secs(timeout_secs)),
    )
    .await
    .map_err(|e| QuoteError(format!("request failed: {e}")))?;
    if !(200..300).contains(&resp.status) {
        return Err(QuoteError(format!("request failed: HTTP {}", resp.status)));
    }
    Ok(String::from_utf8_lossy(&resp.body).into_owned())
}

/// `chart.result[0]` → a [`Quote`].
///
/// Shape: `meta` holds the live quote (price, day high/low/volume, display name), `timestamp[]`
/// holds one epoch-second stamp per bar, and `indicators.quote[0]` holds the index-aligned
/// `open/high/low/close/volume` arrays. Yahoo writes `null` into a bar it has no data for (a
/// halt, or the not-yet-closed session on some feeds), so rows are kept only where the close is
/// a number and every array is filtered through the same index.
///
/// An unknown ticker answers 200 with `result: null` and an `error` object — surfaced as an
/// error rather than an empty chart.
fn parse_chart(body: &str, symbol: &str) -> Result<Quote, QuoteError> {
    let root: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| QuoteError(format!("{symbol}: malformed response ({e})")))?;
    let chart = &root["chart"];
    if let Some(desc) = chart["error"]["description"].as_str() {
        return Err(QuoteError(format!("{symbol}: {desc}")));
    }
    let result = chart["result"]
        .get(0)
        .ok_or_else(|| QuoteError(format!("{symbol}: no data (unknown symbol?)")))?;
    let meta = &result["meta"];
    let quote = &result["indicators"]["quote"][0];

    let stamps = result["timestamp"].as_array().cloned().unwrap_or_default();
    let col = |name: &str| -> Vec<serde_json::Value> {
        quote[name].as_array().cloned().unwrap_or_default()
    };
    let (opens, highs, lows, closes_raw, volumes_raw) = (
        col("open"),
        col("high"),
        col("low"),
        col("close"),
        col("volume"),
    );

    let mut closes = Vec::new();
    let mut volumes = Vec::new();
    let mut dates = Vec::new();
    let mut last_row = None;
    for (i, stamp) in stamps.iter().enumerate() {
        let Some(close) = closes_raw.get(i).and_then(serde_json::Value::as_f64) else {
            continue; // a gap bar — drop the whole row so the arrays stay aligned
        };
        let Some(secs) = stamp.as_i64() else { continue };
        closes.push(close);
        volumes.push(
            volumes_raw
                .get(i)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0),
        );
        dates.push(iso_date(secs));
        last_row = Some(i);
    }
    if closes.is_empty() {
        return Err(QuoteError(format!("{symbol}: no data (unknown symbol?)")));
    }

    // The day's own numbers come from `meta` when present (they track the live session), and
    // fall back to the newest kept bar.
    let bar = |arr: &[serde_json::Value]| -> f64 {
        last_row
            .and_then(|i| arr.get(i))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
    };
    let num = |key: &str| meta[key].as_f64();
    let last = num("regularMarketPrice").unwrap_or_else(|| closes[closes.len() - 1]);
    let prev_close = if closes.len() >= 2 {
        closes[closes.len() - 2]
    } else {
        num("chartPreviousClose").unwrap_or(last)
    };
    let name = meta["shortName"]
        .as_str()
        .or_else(|| meta["longName"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| preset_name(symbol).to_string());

    Ok(Quote {
        name,
        symbol: symbol.to_string(),
        last,
        prev_close,
        open: bar(&opens),
        high: num("regularMarketDayHigh").unwrap_or_else(|| bar(&highs)),
        low: num("regularMarketDayLow").unwrap_or_else(|| bar(&lows)),
        volume: num("regularMarketVolume").unwrap_or_else(|| bar(&volumes_raw)),
        closes,
        volumes,
        dates,
        mock: false,
    })
}

/// Epoch seconds → `YYYY-MM-DD` (UTC). Yahoo stamps each daily bar at its session open, so the
/// UTC date is the trading date for every market the app lists. Hinnant's civil-from-days, so
/// no date crate rides along for one format call.
fn iso_date(epoch_secs: i64) -> String {
    let z = epoch_secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The walkthrough asserts these exact strings — this test pins the mock generator so a
    /// change to it fails HERE, on the host, before it fails on a device.
    #[test]
    fn mock_is_deterministic() {
        let a = mock("AAPL");
        let b = mock("AAPL");
        assert_eq!(a, b);
        assert_eq!(a.closes.len(), 500);
        // Every close is cent-quantized, so 2-decimal formatting is exact everywhere.
        for c in &a.closes {
            assert!((c * 100.0 - (c * 100.0).round()).abs() < 1e-9);
        }
        assert!(a.last > 0.0 && a.low <= a.high);
    }

    /// A real `v8/chart` response, captured 2026-08-11 and trimmed to three bars. Kept
    /// verbatim (field order, float precision, `longName` beside `shortName`) so the parser is
    /// tested against what Yahoo actually sends rather than a tidied-up idea of it.
    const AAPL_JSON: &str = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","shortName":"Apple Inc.","longName":"Apple Inc.","regularMarketPrice":304.91,"chartPreviousClose":309.38,"regularMarketDayHigh":309.97,"regularMarketDayLow":302.79,"regularMarketVolume":34168163},"timestamp":[1786109400,1786368600,1786455000],"indicators":{"quote":[{"open":[311.45001220703125,306.8299865722656,307.75],"high":[314.80999755859375,308.260009765625,309.9700012207031],"low":[310.739990234375,304.6099853515625,302.7900085449219],"close":[313.3299865722656,308.260009765625,304.9100036621094],"volume":[34437200,44812500,34168163]}]}}],"error":null}}"#;

    /// A futures ticker (`=` in the symbol) with a `null` bar — Yahoo's spelling for a session
    /// it has no data for. Captured from the same endpoint and trimmed the same way.
    const GOLD_JSON: &str = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"GC=F","shortName":"Gold Dec 26","regularMarketPrice":4467.3,"chartPreviousClose":4452.1,"regularMarketDayHigh":4478.0,"regularMarketDayLow":4441.2,"regularMarketVolume":1234},"timestamp":[1786109400,1786368600,1786455000],"indicators":{"quote":[{"open":[4450.0,null,4460.1],"high":[4470.0,null,4478.0],"low":[4440.0,null,4441.2],"close":[4452.1,null,4467.3],"volume":[900,null,1234]}]}}],"error":null}}"#;

    /// What an unknown ticker answers: HTTP 200 with `result: null` and an error object.
    const UNKNOWN_JSON: &str = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;

    #[test]
    fn chart_parses_a_real_response() {
        let q = parse_chart(AAPL_JSON, "AAPL").unwrap();
        assert_eq!(q.name, "Apple Inc.");
        assert_eq!(q.symbol, "AAPL");
        assert!(!q.mock);
        // History, oldest → newest, index-aligned across all three vectors.
        assert_eq!(q.closes.len(), 3);
        assert_eq!(q.volumes.len(), 3);
        assert_eq!(q.dates, ["2026-08-07", "2026-08-10", "2026-08-11"]);
        assert_eq!(q.volumes[0], 34_437_200.0);
        // `meta` wins for the live session; prev close is the bar before the newest.
        assert_eq!(q.last, 304.91);
        assert_eq!(q.prev_close, 308.260009765625);
        assert_eq!((q.high, q.low, q.volume), (309.97, 302.79, 34_168_163.0));
        assert_eq!(q.open, 307.75); // newest bar's open
        // The change chip reads from these two, so pin the sign as well as the size.
        assert!(q.change() < 0.0 && q.change_pct() < 0.0);
    }

    #[test]
    fn chart_drops_null_bars_and_keeps_arrays_aligned() {
        let q = parse_chart(GOLD_JSON, "GC=F").unwrap();
        assert_eq!(q.name, "Gold Dec 26"); // no longName on this one — shortName carries it
        // The middle bar is null on every array, so two rows survive, still aligned.
        assert_eq!(q.closes, [4452.1, 4467.3]);
        assert_eq!(q.volumes, [900.0, 1234.0]);
        assert_eq!(q.dates, ["2026-08-07", "2026-08-11"]);
        assert_eq!(q.prev_close, 4452.1);
        assert_eq!(q.open, 4460.1); // the newest KEPT bar, not the null one
    }

    #[test]
    fn unknown_symbol_is_an_error_not_an_empty_chart() {
        let e = parse_chart(UNKNOWN_JSON, "NOPE").unwrap_err();
        assert!(e.0.contains("NOPE"), "{}", e.0);
        assert!(e.0.contains("delisted"), "{}", e.0);
        // A body that is not JSON at all fails the same way rather than panicking.
        assert!(parse_chart("<html>rate limited</html>", "NOPE").is_err());
    }

    #[test]
    fn tickers_survive_the_url_and_legacy_lists_migrate() {
        // `=` must not reach the query string unescaped.
        assert_eq!(percent_encode("GC=F"), "GC%3DF");
        assert_eq!(percent_encode("EURUSD=X"), "EURUSD%3DX");
        assert_eq!(percent_encode("AAPL"), "AAPL");
        // Watchlists saved under the previous provider's spelling.
        assert_eq!(migrate_symbol("AAPL.US"), "AAPL");
        assert_eq!(migrate_symbol("XAUUSD"), "GC=F");
        assert_eq!(migrate_symbol("CL.F"), "CL=F");
        assert_eq!(migrate_symbol("EURUSD"), "EURUSD=X");
        // Already-Yahoo entries are left alone.
        assert_eq!(migrate_symbol("GC=F"), "GC=F");
        assert_eq!(migrate_symbol("MSFT"), "MSFT");
    }

    #[test]
    fn proxy_templates_route_the_url() {
        let url = "https://query1.finance.yahoo.com/v8/finance/chart/AAPL?range=2y&interval=1d";
        // Empty: straight to Yahoo.
        assert_eq!(proxied("", url), url);
        assert_eq!(proxied("   ", url), url);
        // Placeholder: ENCODED, so the target's own `&interval=` stays part of the target
        // rather than becoming a parameter of the proxy (measured: raw substitution 500s).
        // Spelled out rather than read from DEFAULT_WEB_PROXY: this pins the ENCODING RULE, and
        // it must keep passing when the shipped default moves to a relay that takes a path.
        const ENCODED: &str = "https://api.allorigins.win/raw?url=%u";
        let via = proxied(ENCODED, url);
        assert!(
            via.starts_with("https://api.allorigins.win/raw?url=https%3A%2F%2F"),
            "{via}"
        );
        assert!(via.contains("%3Frange%3D2y%26interval%3D1d"), "{via}");
        assert_eq!(
            via.matches('?').count(),
            1,
            "only the proxy's own query survives: {via}"
        );
        // No placeholder: prefix form, target appended verbatim (the cors-anywhere shape).
        assert_eq!(
            proxied("https://proxy.example/", url),
            format!("https://proxy.example/{url}")
        );
        // A ticker with `=` still round-trips through both forms.
        let fut = "https://query1.finance.yahoo.com/v8/finance/chart/GC%3DF?range=2y&interval=1d";
        assert!(
            proxied(ENCODED, fut).contains("GC%253DF"),
            "double-encoded once"
        );
        // Whatever the shipped default is, it must be a template that routes somewhere other
        // than Yahoo — an empty one would send the web build straight into a CORS wall.
        assert!(!DEFAULT_WEB_PROXY.trim().is_empty());
        assert!(!proxied(DEFAULT_WEB_PROXY, url).contains("query1.finance.yahoo.com/v8"));
    }

    /// The path form, which is what an allowlisting relay like `proxy.daybrite.dev` expects.
    /// These are the exact urls the Worker was verified against.
    #[test]
    fn path_templates_carry_the_path_verbatim() {
        let url = "https://query1.finance.yahoo.com/v8/finance/chart/AAPL?range=2y&interval=1d";
        assert_eq!(
            proxied(DAYBRITE_WEB_PROXY, url),
            "https://proxy.daybrite.dev/sites/finance/v8/finance/chart/AAPL?range=2y&interval=1d"
        );
        // The ticker's escape is passed through as-is; re-encoding it would ask the relay for a
        // symbol spelled `GC%3DF`.
        let fut = "https://query1.finance.yahoo.com/v8/finance/chart/GC%3DF?range=2y&interval=1d";
        assert_eq!(
            proxied(DAYBRITE_WEB_PROXY, fut),
            "https://proxy.daybrite.dev/sites/finance/v8/finance/chart/GC%3DF?range=2y&interval=1d"
        );
        // The template decides the host, so the target's own host never appears in the result.
        assert!(!proxied(DAYBRITE_WEB_PROXY, url).contains("yahoo.com"));
        // Degenerate inputs stay in bounds rather than panicking on a slice.
        assert_eq!(path_and_query("https://example.com"), "");
        assert_eq!(path_and_query("https://example.com/"), "");
        assert_eq!(path_and_query("v8/chart?x=1"), "chart?x=1");
    }

    #[test]
    fn iso_date_matches_known_stamps() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(1_786_455_000), "2026-08-11"); // 13:30 UTC = 9:30 ET open
        assert_eq!(iso_date(1_709_164_800), "2024-02-29"); // leap day
    }

    /// Every preset must be a symbol the live path can actually request.
    #[test]
    fn presets_are_named_and_encodable() {
        for s in PRESETS {
            assert_ne!(preset_name(s), s, "{s} has no display name");
            assert!(!percent_encode(s).is_empty());
        }
    }
}

//! Day Tradr's charts, composed from [`day_piece_charts`]: the detail page's price chart with its
//! volume strip and analysis panel, the watchlist's sparklines and performance comparison, the
//! breadth donut, and the range tracks. Every marks closure is a reactive binding: it reads the
//! quote and range signals, so a range tap or a refetch re-records the chart in place.
//!
//! Nothing here touches the canvas directly. A chart is a bag of marks plus scales, guides and a
//! coordinate system, and the crate draws the same display list on every target.

use crate::quotes::{self, Quote};
use day::prelude::*;
use day::reactive::Load;
use day_piece_charts as ch;
use day_piece_charts::{
    AnnotationPosition, Coordinate, Datum, Insets, Interpolation, LegendPosition, Mark, chart,
    date, value,
};

/// The range picker's slices, in trading days (`usize::MAX` = everything the source has).
pub const RANGES: [(&str, usize); 5] = [
    ("1M", 22),
    ("3M", 66),
    ("6M", 132),
    ("1Y", 252),
    ("All", usize::MAX),
];

/// `c` at opacity `a` — chart fills reuse the trend color at several alphas.
fn faded(c: Color, a: f64) -> Color {
    Color::rgba(c.r, c.g, c.b, a)
}

/// The moving-average overlays. Deliberately NOT the trend colors: the averages are reference
/// lines, and reusing green/red would read as a second opinion on the day's direction. Blue and
/// amber stay legible on both themes and are distinguishable in the common color-blindness
/// forms, where green-vs-red is not.
pub const SMA20_COLOR: Color = Color::rgba(0.25, 0.55, 1.0, 0.95);
pub const SMA50_COLOR: Color = Color::rgba(0.98, 0.68, 0.12, 0.95);

/// Price up/flat vs the window's first close → the Stocks green; down → the Stocks red.
pub fn trend_color(up: bool) -> Color {
    if up {
        Color::hex(0x34C759)
    } else {
        Color::hex(0xFF3B30)
    }
}

fn grid_color(dark: bool) -> Color {
    if dark {
        Color::rgba(1.0, 1.0, 1.0, 0.12)
    } else {
        Color::rgba(0.0, 0.0, 0.0, 0.10)
    }
}

/// The margins the price chart, the volume strip and the drawdown chart all share, so their time
/// axes line up: the trailing column holds the price labels, and the leading edge leaves room for
/// the first date label to hang past the plot.
const STACKED_INSETS: Insets = Insets {
    top: 8.0,
    leading: 12.0,
    bottom: 20.0,
    trailing: 56.0,
};

/// The same margins with no room for an axis, for the strip under the chart.
const STRIP_INSETS: Insets = Insets {
    top: 0.0,
    leading: 12.0,
    bottom: 0.0,
    trailing: 56.0,
};

/// The trading days the range picker currently shows.
fn window_days() -> usize {
    RANGES[quotes::range().get().min(RANGES.len() - 1)].1
}

/// The closes in view and the dates aligned with them, or nothing when there is too little to
/// draw a line through.
fn window(q: &Quote, days: usize) -> Option<(&[f64], &[String])> {
    let closes = quotes::tail(&q.closes, days);
    if closes.len() < 2 {
        return None;
    }
    let dates = &q.dates[q.dates.len() - closes.len()..];
    Some((closes, dates))
}

/// The y domain a price chart shows: the window's own extent, padded so the line never touches the
/// plot edge. Pinned rather than inferred because the moving averages can run outside the window's
/// prices, and an average that stretched the axis would flatten the line it is meant to explain.
fn price_domain(closes: &[f64]) -> (f64, f64) {
    let (min, max) = closes
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    let pad = ((max - min) * 0.08).max(max * 0.002);
    (min - pad, max + pad)
}

/// Two decimals, the precision every price on the page shows.
fn price_label(d: &Datum) -> String {
    d.as_continuous()
        .map(|v| format!("{v:.2}"))
        .unwrap_or_default()
}

/// A whole-number percentage with its sign.
fn percent_label(d: &Datum) -> String {
    d.as_continuous()
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_default()
}

/// The big price chart: a gradient area under the price line in the trend color, a dashed
/// reference at the window's first close, the moving-average overlays beneath the line, a halo dot
/// on the latest price, dates along the bottom and price labels down the trailing edge.
pub fn price_chart(quote: Signal<Load<Quote>>) -> impl Piece {
    let overlay = quotes::overlay();
    // What the pointer is over. Owned here rather than by the chart, so the readout under the
    // chart and the guides on it are two views of ONE thing (docs/charts.md "Selection").
    let sel = Signal::new(None);
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let Some((closes, dates)) = window(&q, window_days()) else {
            return Vec::new();
        };
        let (lo, _) = price_domain(closes);
        let first = closes[0];
        let last = closes[closes.len() - 1];
        let line = trend_color(last >= first);
        let price = || value("Series", "Price");
        let mut marks: Vec<Mark> = Vec::with_capacity(closes.len() * 4 + 4);

        // The area fades from the line down to the bottom of the plot. Its lower edge is the
        // pinned domain's floor rather than zero, because an area to zero would be one flat
        // block at this scale.
        for (c, d) in closes.iter().zip(dates) {
            marks.push(
                ch::area(date("Date", d), value("Price", *c))
                    .y_range(value("Price", lo), value("Price", *c))
                    .gradient(faded(line, 0.30), faded(line, 0.02)),
            );
        }
        // Dashed reference at the window's first close.
        marks.push(
            ch::rule_y(value("Price", first))
                .foreground(Color::rgba(0.5, 0.5, 0.5, 0.55))
                .line_width(1.0)
                .dash([5.0, 5.0]),
        );
        // Moving-average overlays UNDER the price line, so the price always reads on top. Each
        // starts where it exists (an SMA has no value until it has N samples), and both are
        // computed over the FULL history rather than the visible window — a 50-day average of
        // a 22-day window would otherwise be a 22-day average wearing the wrong label. A value
        // outside the pinned domain is clipped by the chart rather than stretching it.
        if overlay.get() {
            let from = q.closes.len() - closes.len();
            for (period, name, color) in [
                (20usize, "20-day", SMA20_COLOR),
                (50usize, "50-day", SMA50_COLOR),
            ] {
                let series = quotes::sma_series(&q.closes, period);
                for (i, d) in dates.iter().enumerate() {
                    if let Some(v) = series[from + i] {
                        marks.push(
                            ch::line(date("Date", d), value("Price", v))
                                .by_series(value("Series", name))
                                .foreground(color)
                                .line_width(1.25),
                        );
                    }
                }
            }
        }
        // The price line itself, as its own series so it never joins an overlay's path.
        for (c, d) in closes.iter().zip(dates) {
            marks.push(
                ch::line(date("Date", d), value("Price", *c))
                    .by_series(price())
                    .foreground(line)
                    .line_width(2.0)
                    .rounded(),
            );
        }
        // Latest price: a soft halo + solid dot. Symbol sizes are areas (πr²): a 7pt halo around
        // a 3.5pt dot.
        let end = dates[dates.len() - 1].as_str();
        marks.push(
            ch::point(date("Date", end), value("Price", last))
                .foreground(faded(line, 0.25))
                .symbol_size(154.0),
        );
        marks.push(
            ch::point(date("Date", end), value("Price", last))
                .foreground(line)
                .symbol_size(38.5),
        );
        marks
    })
    // The domain is a property of the chart, not of a mark, and it has to follow the range
    // picker like the marks do: the closure is re-read inside the chart's own binding.
    .y_domain_with(move || windowed_price_domain(quote))
    .y_axis_trailing()
    .y_format(price_label)
    .x_tick_count(4)
    .x_label("")
    .y_label("")
    .legend(LegendPosition::Hidden)
    // A price chart snaps along x: a scrub reads every series at one date — the close and any
    // overlay together — which is the whole reason to scrub a time series at all. Hover on a
    // desktop, tap or drag on a phone; the chart wires all three.
    .select(sel)
    .snap(ch::select::Snap::NearestX)
    .guides(ch::select::Guides::RULE)
    .plot_insets(STACKED_INSETS)
    // After every Chart builder (`Decorated` does not forward them) and before `.height`/`.grow_w`
    // (an id after a wrapper tags the wrapper, and a dayscript tap would miss the canvas).
    .id("price-chart")
    .height(280.0)
    .grow_w()
}

/// The padded extent of the closes in view, for a chart that pins its y axis to them.
fn windowed_price_domain(quote: Signal<Load<Quote>>) -> Option<(f64, f64)> {
    quote.with(|l| {
        l.ready()
            .and_then(|q| window(q, window_days()).map(|(c, _)| price_domain(c)))
    })
}

/// The first and last instants in view, for a strip that shares the price chart's time axis.
fn windowed_date_domain(quote: Signal<Load<Quote>>) -> Option<(f64, f64)> {
    quote.with(|l| {
        l.ready().and_then(|q| {
            let (_, dates) = window(q, window_days())?;
            Some((
                ch::parse_iso_date(&dates[0])?,
                ch::parse_iso_date(&dates[dates.len() - 1])?,
            ))
        })
    })
}

/// The volume strip under the chart: one bar per session on the same time axis as the price
/// chart, trend-colored by that session's direction. The x domain is pinned to the chart's own
/// so the two line up bar for bar; the bars at either end are cut in half by that pin, which is
/// where the price line ends too.
pub fn volume_strip(quote: Signal<Load<Quote>>) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let Some((closes, dates)) = window(&q, window_days()) else {
            return Vec::new();
        };
        let volumes = quotes::tail(&q.volumes, closes.len());
        closes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let up = i == 0 || *c >= closes[i - 1];
                ch::bar(date("Date", &dates[i]), value("Volume", volumes[i]))
                    .foreground(faded(trend_color(up), 0.55))
            })
            .collect()
    })
    .x_domain_with(move || windowed_date_domain(quote))
    .bare()
    .plot_insets(STRIP_INSETS)
    .height(56.0)
    .grow_w()
}

/// How far the price sits below its running peak, as an area in the loss color. The reading a
/// price line hides: two symbols can end a year at the same gain and have spent it very
/// differently on the way.
pub fn drawdown_chart(quote: Signal<Load<Quote>>) -> impl Piece {
    let red = trend_color(false);
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let Some((closes, dates)) = window(&q, window_days()) else {
            return Vec::new();
        };
        let dd = quotes::drawdown_series(closes);
        let mut marks: Vec<Mark> = Vec::with_capacity(closes.len() * 2);
        for (v, d) in dd.iter().zip(dates) {
            marks.push(
                ch::area(date("Date", d), value("Drawdown", *v))
                    .gradient(faded(red, 0.06), faded(red, 0.38)),
            );
        }
        for (v, d) in dd.iter().zip(dates) {
            marks.push(
                ch::line(date("Date", d), value("Drawdown", *v))
                    .foreground(red)
                    .line_width(1.5)
                    .rounded(),
            );
        }
        marks
    })
    .y_axis_trailing()
    .y_format(percent_label)
    .x_tick_count(4)
    .x_label("")
    .y_label("")
    .plot_insets(STACKED_INSETS)
    .height(200.0)
    .grow_w()
}

/// The distribution of one-session moves in the window: a histogram of daily returns, losses in
/// red and gains in green, with a dashed rule at zero. Each bin is a rectangle spanning its own
/// edges on a continuous axis, so the axis labels itself in round percentages rather than naming
/// every bin.
pub fn returns_histogram(quote: Signal<Load<Quote>>, axis_label: String) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let Some((closes, _)) = window(&q, window_days()) else {
            return Vec::new();
        };
        let hist = quotes::return_histogram(&quotes::daily_returns(closes), 9);
        let mut marks: Vec<Mark> = Vec::with_capacity(hist.bins.len() + 1);
        for (lo, count) in &hist.bins {
            let hi = lo + hist.width;
            marks.push(
                ch::rect(value("Return", *lo), value("Sessions", *count as f64))
                    .x_range(value("Return", *lo), value("Return", hi))
                    .foreground(faded(trend_color(*lo >= 0.0), 0.85))
                    .corner_radius(2.0)
                    .width(ch::Dimension::Inset(1.0)),
            );
        }
        marks.push(
            ch::rule_x(value("Return", 0.0))
                .foreground(Color::rgba(0.5, 0.5, 0.5, 0.7))
                .line_width(1.0)
                .dash([4.0, 4.0]),
        );
        marks
    })
    .x_format(|d| {
        d.as_continuous()
            .map(|v| {
                // Zero is an edge, not a move; it takes no sign.
                if v.abs() < 1e-9 {
                    "0%".to_string()
                } else {
                    format!("{v:+.1}%")
                }
            })
            .unwrap_or_default()
    })
    .x_label(axis_label)
    .y_label("")
    .y_tick_count(4)
    .height(200.0)
    .grow_w()
}

/// A calendar of monthly returns: months across, years down, each cell colored on the diverging
/// ramp around zero and printed with its percentage. Two categorical axes, which is what a heat
/// map is in the grammar.
pub fn monthly_heat_map(quote: Signal<Load<Quote>>) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let dark = day::dark_mode();
        let months = quotes::monthly_returns(&q.closes, &q.dates);
        // The ramp's reach: the biggest move either way, and never so small that a quiet year
        // is painted in the deepest colors.
        let reach = months
            .iter()
            .map(|(_, _, r)| r.abs())
            .fold(1.0f64, f64::max);
        months
            .iter()
            .map(|(year, month, r)| {
                let t = 0.5 + r / (2.0 * reach);
                // Light text on the saturated ends, the chart's own label color near neutral.
                let ink = if (t - 0.5).abs() > 0.28 {
                    Color::WHITE
                } else if dark {
                    Color::rgba(0.0, 0.0, 0.0, 0.8)
                } else {
                    Color::rgba(0.0, 0.0, 0.0, 0.75)
                };
                ch::rect(
                    value("Month", quotes::MONTH_NAMES[(*month as usize - 1).min(11)]),
                    value("Year", year.to_string()),
                )
                .foreground(ch::diverging(t))
                .width(ch::Dimension::Inset(1.0))
                .height(ch::Dimension::Inset(1.0))
                .corner_radius(3.0)
                .annotation(AnnotationPosition::Overlay, format!("{r:+.1}"))
                .annotation_color(ink)
            })
            .collect()
    })
    .x_categories(quotes::MONTH_NAMES)
    .no_grid()
    .x_label("")
    .y_label("")
    .label_size(10.0)
    .height(140.0)
    .grow_w()
}

/// A watchlist-row sparkline: the last month as a tiny line + soft fill, trend-colored.
pub fn sparkline(quote: Signal<Load<Quote>>) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let closes = quotes::tail(&q.closes, 22);
        if closes.len() < 2 {
            return Vec::new();
        }
        let (lo, _) = price_domain(closes);
        // Colored by the DAY change, matching the row's chip (the Stocks convention), not by
        // the sparkline window's own trend.
        let line = trend_color(q.change() >= 0.0);
        let mut marks: Vec<Mark> = Vec::with_capacity(closes.len() * 2);
        // A thumbnail, so the spline is decoration rather than data. The domain is pinned, which
        // clips the curve's overshoot at a spike to the plot instead of painting the row beside.
        for (i, c) in closes.iter().enumerate() {
            marks.push(
                ch::area(value("Day", i as f64), value("Price", *c))
                    .y_range(value("Price", lo), value("Price", *c))
                    .interpolation(Interpolation::CatmullRom)
                    .gradient(faded(line, 0.25), faded(line, 0.0)),
            );
        }
        for (i, c) in closes.iter().enumerate() {
            marks.push(
                ch::line(value("Day", i as f64), value("Price", *c))
                    .interpolation(Interpolation::CatmullRom)
                    .foreground(line)
                    .line_width(1.5)
                    .rounded(),
            );
        }
        marks
    })
    .y_domain_with(move || {
        quote.with(|l| {
            l.ready().and_then(|q| {
                let closes = quotes::tail(&q.closes, 22);
                (closes.len() >= 2).then(|| price_domain(closes))
            })
        })
    })
    .bare()
    .plot_insets(Insets::uniform(1.0))
    .frame(84.0, 30.0)
}

/// Every tracked symbol over the shared range, each indexed to 100 at the window's first close,
/// so a 3,000-dollar future and a one-dollar currency pair share one axis and the eye reads
/// relative performance straight off the lines. One series per symbol on the crate's own
/// colorblind-safe palette, named in the legend.
pub fn performance_chart(list: Signal<Vec<String>>) -> impl Piece {
    chart(move || {
        let days = window_days();
        let mut marks: Vec<Mark> = Vec::new();
        for symbol in list.get() {
            let quote = quotes::resource_for(&symbol).signal();
            let Some(q) = quote.with(|l| l.ready().cloned()) else {
                continue;
            };
            let Some((closes, dates)) = window(&q, days) else {
                continue;
            };
            let first = closes[0];
            if first <= 0.0 {
                continue;
            }
            for (c, d) in closes.iter().zip(dates) {
                marks.push(
                    ch::line(date("Date", d), value("Indexed", c / first * 100.0))
                        .by_series(value("Symbol", symbol.clone()))
                        .line_width(1.5)
                        .rounded(),
                );
            }
        }
        if !marks.is_empty() {
            marks.push(
                ch::rule_y(value("Indexed", 100.0))
                    .foreground(Color::rgba(0.5, 0.5, 0.5, 0.55))
                    .line_width(1.0)
                    .dash([5.0, 5.0]),
            );
        }
        marks
    })
    .y_axis_trailing()
    .y_format(|d| {
        d.as_continuous()
            .map(|v| format!("{v:.0}"))
            .unwrap_or_default()
    })
    .x_tick_count(4)
    .x_label("")
    .y_label("")
    .legend(LegendPosition::Bottom)
    .height(240.0)
    .grow_w()
}

/// Risk against return: one point per watchlist symbol, volatility across, annualized return up.
///
/// The chart a price line cannot be — it compares symbols on two numbers at once, and the shape of
/// the cloud is the reading: a point up and to the LEFT earned more for less movement. Interactive
/// because a scatter without it is unreadable the moment two points sit close: hovering (or, on a
/// phone, tapping) names the symbol under the pointer and its two figures.
pub fn risk_return_scatter(
    list: Signal<Vec<String>>,
    sel: Signal<Option<ch::select::Selection>>,
) -> impl Piece {
    chart(move || {
        let days = window_days();
        let mut marks: Vec<Mark> = Vec::new();
        for symbol in list.get() {
            let quote = quotes::resource_for(&symbol).signal();
            let Some(q) = quote.with(|l| l.ready().cloned()) else {
                continue;
            };
            let Some((closes, _)) = window(&q, days) else {
                continue;
            };
            let Some((annual, vol)) = quotes::risk_return(closes) else {
                continue;
            };
            marks.push(
                ch::point(value("Volatility", vol), value("Return", annual))
                    .by_series(value("Symbol", symbol.clone()))
                    .symbol_size(90.0),
            );
        }
        if !marks.is_empty() {
            // Break-even, so "made money" and "lost money" are two halves of the picture rather
            // than a number to read off the axis.
            marks.push(
                ch::rule_y(value("Return", 0.0))
                    .foreground(Color::rgba(0.5, 0.5, 0.5, 0.55))
                    .line_width(1.0)
                    .dash([5.0, 5.0]),
            );
        }
        marks
    })
    .x_format(percent_label)
    .y_format(percent_label)
    .y_axis_trailing()
    .x_tick_count(5)
    .legend(LegendPosition::Bottom)
    .select(sel)
    .snap(ch::select::Snap::NearestMark)
    .guides(ch::select::Guides::CROSSHAIR)
    .height(260.0)
    .grow_w()
}

/// How the watchlist moves together: pairwise correlation of daily returns, as a matrix.
///
/// Two discrete axes and a diverging ramp — the one chart here whose value is entirely in its
/// COLOR, which is why it needs the selection: a reader can see that a cell is blue without being
/// able to say whether that is 0.3 or 0.6, and the label under the pointer says which.
pub fn correlation_matrix(
    list: Signal<Vec<String>>,
    sel: Signal<Option<ch::select::Selection>>,
) -> impl Piece {
    chart(move || {
        let days = window_days();
        let symbols = list.get();
        // Each symbol's returns once, not once per pair.
        let series: Vec<(String, Vec<f64>)> = symbols
            .iter()
            .filter_map(|symbol| {
                let quote = quotes::resource_for(symbol).signal();
                let q = quote.with(|l| l.ready().cloned())?;
                let (closes, _) = window(&q, days)?;
                Some((symbol.clone(), quotes::daily_returns(closes)))
            })
            .collect();
        let mut marks: Vec<Mark> = Vec::new();
        for (a, ra) in &series {
            for (b, rb) in &series {
                let Some(c) = quotes::correlation(ra, rb) else {
                    continue;
                };
                marks.push(
                    ch::rect(value("Symbol", a.clone()), value("Against", b.clone()))
                        .foreground(correlation_color(c))
                        .corner_radius(2.0),
                );
            }
        }
        marks
    })
    .legend(LegendPosition::Hidden)
    .select(sel)
    .snap(ch::select::Snap::NearestMark)
    .guides(ch::select::Guides::CROSSHAIR)
    .height(260.0)
    .grow_w()
}

/// A correlation as a color: the loss hue for negative, the gain hue for positive, and near-white
/// through zero — a DIVERGING ramp, because zero is a meaningful middle here rather than one end
/// of a range.
fn correlation_color(c: f64) -> Color {
    let t = c.abs().clamp(0.0, 1.0);
    let end = trend_color(c >= 0.0);
    Color::rgba(end.r, end.g, end.b, 0.12 + 0.78 * t)
}

/// Where the symbol actually traded: volume summed into price bands, drawn sideways so the bands
/// line up with the price axis a reader has just been looking at.
///
/// A horizontal bar chart — the y axis discrete, the x continuous — which is the same `bar` mark
/// with its channels the other way round.
pub fn volume_profile(
    quote: Signal<Load<Quote>>,
    sel: Signal<Option<ch::select::Selection>>,
) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let Some((closes, _)) = window(&q, window_days()) else {
            return Vec::new();
        };
        let volumes = quotes::tail(&q.volumes, closes.len());
        let bands = quotes::volume_by_price(closes, volumes, 14);
        let peak = bands.iter().map(|(_, _, v)| *v).fold(0.0f64, f64::max);
        bands
            .iter()
            .filter(|(_, _, v)| *v > 0.0)
            .map(|(lo, hi, v)| {
                let mid = (lo + hi) / 2.0;
                // The heaviest band in full strength, the rest faded by share — so the level the
                // symbol traded at most reads first.
                let share = if peak > 0.0 { v / peak } else { 0.0 };
                ch::bar(value("Volume", *v), value("Price", format!("{mid:.0}")))
                    .foreground(faded(SMA50_COLOR, 0.25 + 0.6 * share))
            })
            .collect()
    })
    .x_tick_count(3)
    .legend(LegendPosition::Hidden)
    .select(sel)
    .snap(ch::select::Snap::NearestMark)
    .guides(ch::select::Guides::RULE)
    .height(220.0)
    .grow_w()
}

/// Today's breadth as a ring: advancing symbols in green, declining in red. A stacked bar in
/// polar coordinates, which is what a donut is.
pub fn breadth_donut(list: Signal<Vec<String>>) -> impl Piece {
    chart(move || {
        let Some(b) = quotes::breadth(&list.get()) else {
            return Vec::new();
        };
        vec![
            ch::sector(value("Symbols", b.up as f64))
                .by_series(value("Direction", "Advancing"))
                .foreground(trend_color(true))
                .angular_inset(1.5),
            ch::sector(value("Symbols", b.down as f64))
                .by_series(value("Direction", "Declining"))
                .foreground(trend_color(false))
                .angular_inset(1.5),
        ]
    })
    .coordinate(Coordinate::donut(0.6))
    .bare()
    .frame(44.0, 44.0)
}

/// A low → high band with the current price marked on it: the reading a stats cell cannot give,
/// which is where today's price sits *within* a range. Used for both the session range and the
/// 52-week range on the detail page.
///
/// The TRACK only — the endpoint numbers are real labels beside it (see `detail::range_row`),
/// because canvas text carries neither the reader's font scale nor RTL mirroring. Two rounded
/// rules on one x axis whose domain is the band itself, and a two-mark dot where the price is.
pub fn range_bar(
    quote: Signal<Load<Quote>>,
    lo: impl Fn(&Quote) -> f64 + 'static,
    hi: impl Fn(&Quote) -> f64 + 'static,
) -> impl Piece {
    chart(move || {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return Vec::new();
        };
        let dark = day::dark_mode();
        let (lo_v, hi_v) = (lo(&q), hi(&q));
        let at = lo_v + q.position_in(lo_v, hi_v) * (hi_v - lo_v);
        let line = trend_color(q.change() >= 0.0);
        let level = || value("Level", 0.0);
        vec![
            // The track: the whole band in a neutral fill, so the marker reads as the
            // information and the band as the backdrop.
            ch::rule_y(level())
                .x_range(value("Price", lo_v), value("Price", hi_v))
                .foreground(grid_color(dark))
                .line_width(5.0)
                .rounded(),
            // The filled portion, in the day's trend color, from the low up to the price.
            ch::rule_y(level())
                .x_range(value("Price", lo_v), value("Price", at))
                .foreground(faded(line, 0.55))
                .line_width(5.0)
                .rounded(),
            ch::point(value("Price", at), level())
                .foreground(faded(line, 0.22))
                .symbol_size(113.0),
            ch::point(value("Price", at), level())
                .foreground(line)
                .symbol_size(38.5),
        ]
    })
    .bare()
    .plot_insets(Insets {
        top: 0.0,
        leading: 6.0,
        bottom: 0.0,
        trailing: 6.0,
    })
    .height(20.0)
    .grow_w()
}

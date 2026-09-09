//! The watchlist: the day's breadth with its donut, every symbol's performance on one indexed
//! chart over the shared range, then one rich card per tracked symbol — name, sparkline, price,
//! and a colored change chip — reconciled by `each` (rows keep their state as the list changes)
//! and tappable through to the symbol's detail page.

use crate::charts;
use crate::quotes;
use crate::res;
use day::prelude::*;

/// The chip's text in the current display mode, sign folded into the number:
/// `+1.24 (0.62%)` / `−3.10%` / `+1.24`.
pub fn change_text(q: &quotes::Quote, mode: quotes::ChipMode) -> String {
    let sign = if q.change() >= 0.0 { "+" } else { "−" };
    let (abs, pct) = (q.change().abs(), q.change_pct().abs());
    match mode {
        quotes::ChipMode::Both => format!("{sign}{abs:.2} ({pct:.2}%)"),
        quotes::ChipMode::Percent => format!("{sign}{pct:.2}%"),
        quotes::ChipMode::Absolute => format!("{sign}{abs:.2}"),
    }
}

/// The change chip: white text on the trend color, rounded — the Stocks signature. What it
/// reads (absolute, percent, or both) is app-wide state driven by [`chip_mode_button`], so
/// every chip on every page changes together.
pub fn change_chip(quote: Signal<day::reactive::Load<quotes::Quote>>, id: String) -> impl Piece {
    let mode = quotes::chip_mode();
    label(move || {
        quote.with(|l| {
            l.ready()
                .map(|q| change_text(q, mode.get()))
                .unwrap_or_default()
        })
    })
    .font(Font::Callout)
    .color(Color::WHITE)
    // `.id` before the wrapping decorators: padding/background each wrap the label in a new
    // node, and an id applied after would tag the wrapper instead of the label the script
    // asserts on.
    .id(id)
    .padding(Insets {
        top: 3.0,
        bottom: 3.0,
        leading: 8.0,
        trailing: 8.0,
    })
    .background(move || {
        quote.with(|l| {
            l.ready()
                .map(|q| charts::trend_color(q.change() >= 0.0))
                .unwrap_or(Color::rgba(0.5, 0.5, 0.5, 0.4))
        })
    })
    .corner_radius(7.0)
}

/// The chip-mode control. Apple Stocks cycles this by tapping a chip, which does not work
/// here: a watchlist chip sits inside a row whose own tap navigates to the detail page, so a
/// tap would have to both cycle and navigate. An explicit button says what the next reading
/// will be and belongs to no row.
fn chip_mode_button() -> impl Piece {
    let mode = quotes::chip_mode();
    button(move || match mode.get() {
        quotes::ChipMode::Both => res::str::chip_both().format(),
        quotes::ChipMode::Percent => res::str::chip_percent().format(),
        quotes::ChipMode::Absolute => res::str::chip_absolute().format(),
    })
    .action(quotes::cycle_chip_mode)
    .id("chip-mode")
}

/// Is this window too narrow to put controls side by side? Tracked, so crossing the breakpoint
/// (a phone rotating, a desktop window dragged narrow) re-lays the controls in place. A backend
/// that reports no class is treated as roomy — every one that does report is a real measurement.
pub(crate) fn compact_width() -> bool {
    day::size_class().is_some_and(|c| !c.prefers_split())
}

/// Today's breadth over the whole watchlist: how many symbols are up, how many down, and the
/// day's biggest mover each way — the summary a list of rows cannot give at a glance.
fn breadth_strip(list: Signal<Vec<String>>) -> impl Piece {
    // BOTH lines take closures: the mover captions carry a live percentage, so a
    // once-formatted `LocalizedText` would freeze at the placeholder the first build saw.
    let cell = |value: Box<dyn Fn() -> String>,
                caption: Box<dyn Fn() -> String>,
                tint: Option<Color>,
                id: &str| {
        // Color before `.id`: `.id` yields an AnyPiece, and the tint has to land on the label.
        let mut v = label(value).font(Font::Subheadline).bold();
        if let Some(c) = tint {
            v = v.color(c);
        }
        column((v.id(id.to_string()), label(caption).font(Font::Caption2)))
            .spacing(1.0)
            .align(HAlign::Leading)
            .grow_w()
            .any()
    };
    // The mover cells carry the SYMBOL as their value and the percentage in the caption. Both
    // on the value line wraps to two lines on a phone, which drops that cell's caption below
    // the other three and leaves the strip on a ragged baseline.
    let mover_symbol = |best: bool| {
        let f = move || {
            quotes::breadth(&list.get())
                .and_then(|b| if best { b.best } else { b.worst })
                .map(|(s, _)| s)
                .unwrap_or_else(|| "—".to_string())
        };
        Box::new(f) as Box<dyn Fn() -> String>
    };
    let mover_pct = move |best: bool| {
        quotes::breadth(&list.get())
            .and_then(|b| if best { b.best } else { b.worst })
            .map(|(_, pct)| format!("{pct:+.2}%"))
            .unwrap_or_else(|| "—".to_string())
    };
    row((
        // The same two counts as a ring, so the day's balance reads before the numbers do.
        charts::breadth_donut(list).id("breadth-donut"),
        cell(
            Box::new(move || {
                quotes::breadth(&list.get())
                    .map(|b| b.up.to_string())
                    .unwrap_or_else(|| "—".to_string())
            }),
            Box::new(|| res::str::breadth_up().format()),
            Some(charts::trend_color(true)),
            "breadth-up",
        ),
        cell(
            Box::new(move || {
                quotes::breadth(&list.get())
                    .map(|b| b.down.to_string())
                    .unwrap_or_else(|| "—".to_string())
            }),
            Box::new(|| res::str::breadth_down().format()),
            Some(charts::trend_color(false)),
            "breadth-down",
        ),
        cell(
            mover_symbol(true),
            Box::new(move || res::str::breadth_best(mover_pct(true)).format()),
            None,
            "breadth-best",
        ),
        cell(
            mover_symbol(false),
            Box::new(move || res::str::breadth_worst(mover_pct(false)).format()),
            None,
            "breadth-worst",
        ),
    ))
    .spacing(10.0)
    .align(VAlign::Center)
    .padding(Insets {
        top: 10.0,
        bottom: 10.0,
        leading: 14.0,
        trailing: 14.0,
    })
    .background(Color::rgba(0.5, 0.5, 0.5, 0.10))
    .corner_radius(12.0)
    .id("breadth")
}

/// Every symbol's move over the shared range on one axis, each indexed to 100 at the window's
/// start, so the lines read as relative performance whatever the instruments' prices. The range
/// picker is the detail page's own signal: the range chosen here is the one a symbol opens on.
fn performance_card(list: Signal<Vec<String>>) -> impl Piece {
    let ranges: Vec<String> = charts::RANGES.iter().map(|(n, _)| n.to_string()).collect();
    let title = label(res::str::performance_title()).font(Font::Headline);
    let range_picker = picker(ranges, quotes::range())
        .segmented()
        .id("wl-range-picker");
    // The title and a five-segment picker do not fit side by side on a phone; there they stack.
    let header = if compact_width() {
        column((title, range_picker))
            .spacing(8.0)
            .align(HAlign::Leading)
            .grow_w()
            .any()
    } else {
        row((title, spacer(), range_picker))
            .spacing(10.0)
            .align(VAlign::Center)
            .grow_w()
            .any()
    };
    column((
        header,
        charts::performance_chart(list).id("performance-chart"),
    ))
    .spacing(8.0)
    .align(HAlign::Leading)
    .padding(Insets {
        top: 10.0,
        bottom: 10.0,
        leading: 14.0,
        trailing: 14.0,
    })
    .background(Color::rgba(0.5, 0.5, 0.5, 0.10))
    .corner_radius(12.0)
    .id("performance")
}

fn row_card(symbol: String) -> impl Piece {
    let quote = quotes::resource_for(&symbol).signal();
    let nav_to = symbol.clone();
    let title = symbol.clone();
    row((
        column((
            label(title).font(Font::Headline).bold(),
            label(move || quote.with(|l| l.ready().map(|q| q.name.clone()).unwrap_or_default()))
                .font(Font::Footnote),
        ))
        .spacing(2.0)
        .align(HAlign::Leading)
        .grow_w(),
        charts::sparkline(quote),
        column((
            label(move || {
                quote.with(|l| {
                    l.ready()
                        .map(|q| format!("{:.2}", q.last))
                        .unwrap_or_else(|| "…".to_string())
                })
            })
            .font(Font::Title3)
            .bold()
            .id(format!("wl-price-{symbol}")),
            change_chip(quote, format!("wl-chg-{symbol}")),
        ))
        .spacing(3.0)
        .align(HAlign::Trailing),
    ))
    .spacing(12.0)
    .padding(Insets {
        top: 10.0,
        bottom: 10.0,
        leading: 14.0,
        trailing: 14.0,
    })
    .background(Color::rgba(0.5, 0.5, 0.5, 0.10))
    .corner_radius(12.0)
    .on_tap(move || {
        crate::open_symbol(&nav_to);
    })
    .id(format!("wl-row-{symbol}"))
}

pub fn watchlist_page() -> impl Piece {
    let list = quotes::symbols();
    let sort = quotes::sort();
    // The sort picker's labels, in `Sort::ALL` order.
    let sort_names: Vec<String> = vec![
        res::str::sort_manual().format(),
        res::str::sort_name().format(),
        res::str::sort_change().format(),
    ];
    // `picker` binds a usize; map it through `Sort::ALL` so the enum stays the source of truth.
    let sort_ix = Signal::new(
        quotes::Sort::ALL
            .iter()
            .position(|s| *s == sort.get_untracked())
            .unwrap_or(0),
    );
    Effect::new(move || {
        let ix = sort_ix.get();
        let chosen = quotes::Sort::ALL[ix.min(quotes::Sort::ALL.len() - 1)];
        if chosen != sort.get_untracked() {
            quotes::set_sort(chosen);
        }
    });

    scroll(
        column((
            label(res::str::watchlist_title())
                .font(Font::Title)
                .bold()
                .id("watchlist-title"),
            label(move || {
                if quotes::is_mock() {
                    res::str::data_mock().format()
                } else {
                    res::str::data_live().format()
                }
            })
            .font(Font::Caption)
            .id("data-source"),
            // Empty watchlist: say so and point at the page that fixes it, rather than
            // rendering a bare heading over nothing.
            when(
                move || list.get().is_empty(),
                || {
                    column((
                        label(res::str::watchlist_empty()).font(Font::Callout),
                        label(res::str::watchlist_empty_hint()).font(Font::Footnote),
                    ))
                    .spacing(4.0)
                    .align(HAlign::Leading)
                    .padding(14.0)
                    .background(Color::rgba(0.5, 0.5, 0.5, 0.10))
                    .corner_radius(12.0)
                    .id("watchlist-empty")
                },
            ),
            when(move || !list.get().is_empty(), move || breadth_strip(list)),
            when(
                move || !list.get().is_empty(),
                move || performance_card(list),
            ),
            // List-wide controls, laid out for the width available. Side by side needs room
            // for a three-segment picker AND the chip button; at compact width that overflows —
            // on a 392dp phone the localized segments alone eat the row and the button lands
            // off-screen — so there they stack instead. Built ONCE and placed either way, so
            // each id has a single call site.
            when(
                move || !list.get().is_empty(),
                move || {
                    let sorter = picker(sort_names.clone(), sort_ix)
                        .segmented()
                        .id("sort-picker");
                    let mode = chip_mode_button();
                    if compact_width() {
                        column((sorter, mode))
                            .spacing(8.0)
                            .align(HAlign::Leading)
                            .grow_w()
                            .any()
                    } else {
                        row((sorter, spacer(), mode))
                            .spacing(10.0)
                            .align(VAlign::Center)
                            .grow_w()
                            .any()
                    }
                },
            ),
            each(
                items(
                    move || quotes::sorted_symbols(list.get(), sort.get()),
                    |s| s.clone(),
                ),
                |slot| row_card(slot.key()),
            ),
            label(res::str::data_attribution()).font(Font::Caption2),
        ))
        .spacing(10.0)
        .align(HAlign::Leading)
        .padding(16.0),
    )
    .grow()
}

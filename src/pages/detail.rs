//! One instrument's page: big price header with change chip, the shared range picker, the price
//! chart + volume strip, an analysis panel with three readings of the same history, the range
//! tracks, and a three-column stats grid — every value bound to the symbol's reactive
//! [`Resource`] so a refetch or range tap updates in place.

use crate::charts;
use crate::pages::watchlist::{change_chip, compact_width};
use crate::quotes;
use crate::res;
use day::prelude::*;
use day::reactive::Load;

fn fmt_volume(v: f64) -> String {
    if v >= 1e9 {
        format!("{:.2}B", v / 1e9)
    } else if v >= 1e6 {
        format!("{:.2}M", v / 1e6)
    } else if v >= 1e3 {
        format!("{:.1}K", v / 1e3)
    } else {
        format!("{v:.0}")
    }
}

/// One stats cell: caption over value, value bound to the quote.
fn stat(
    title: LocalizedText,
    quote: Signal<Load<quotes::Quote>>,
    value: impl Fn(&quotes::Quote) -> String + 'static,
) -> impl Piece {
    column((
        label(title).font(Font::Caption),
        label(move || quote.with(|l| l.ready().map(&value).unwrap_or_default()))
            .font(Font::Callout)
            .bold(),
    ))
    .spacing(1.0)
    .align(HAlign::Leading)
    .grow_w()
}

/// A titled range bar: caption over the low→high track with the price marked on it.
/// `Copy` on the accessors so the same one can feed the track and its endpoint label; the call
/// sites pass non-capturing closures, which are Copy.
fn range_row(
    title: LocalizedText,
    quote: Signal<Load<quotes::Quote>>,
    lo: impl Fn(&quotes::Quote) -> f64 + Copy + 'static,
    hi: impl Fn(&quotes::Quote) -> f64 + Copy + 'static,
    id: &str,
) -> impl Piece {
    let lo_label = label(move || {
        quote.with(|l| {
            l.ready()
                .map(|q| format!("{:.2}", lo(q)))
                .unwrap_or_default()
        })
    })
    .font(Font::Caption2);
    let hi_label = label(move || {
        quote.with(|l| {
            l.ready()
                .map(|q| format!("{:.2}", hi(q)))
                .unwrap_or_default()
        })
    })
    .font(Font::Caption2);
    column((
        label(title).font(Font::Caption),
        charts::range_bar(quote, lo, hi).id(id.to_string()),
        row((lo_label, spacer(), hi_label)).grow_w(),
    ))
    .spacing(2.0)
    .align(HAlign::Leading)
    .grow_w()
}

/// One overlay legend entry: a short colored rule beside its label.
fn legend(color: Color, text: LocalizedText) -> impl Piece {
    row((
        rounded_rectangle(1.5).fill(color).frame(14.0, 3.0),
        label(text).font(Font::Caption2),
    ))
    .spacing(5.0)
    .align(VAlign::Center)
}

/// The analysis panel under the chart: how far the price sits below its peak, how its daily
/// moves are distributed, or how each calendar month went — one at a time, picked by a segmented
/// control. Three charts rather than one with switching marks, because each is a different
/// composition — a time axis, a continuous histogram axis, two categorical axes — and the axes
/// are properties of the chart, not of its marks. The choice is page-local: a way of looking at
/// this symbol, not a setting.
fn analysis_panel(quote: Signal<Load<quotes::Quote>>) -> impl Piece {
    let picked = Signal::new(0usize);
    let names: Vec<String> = vec![
        res::str::analysis_drawdown().format(),
        res::str::analysis_returns().format(),
        res::str::analysis_monthly().format(),
    ];
    let title = label(res::str::analysis_label()).font(Font::Callout);
    let control = picker(names, picked).segmented().id("analysis-picker");
    // The caption and a three-segment picker share a row where there is room; on a phone the
    // picker takes the whole width, or its last segment lands off-screen.
    let header = if compact_width() {
        column((title, control))
            .spacing(6.0)
            .align(HAlign::Leading)
            .grow_w()
            .any()
    } else {
        row((title, spacer(), control))
            .spacing(10.0)
            .align(VAlign::Center)
            .grow_w()
            .any()
    };
    column((
        header,
        when(
            move || picked.get() == 1,
            move || {
                charts::returns_histogram(quote, res::str::axis_return().format())
                    .id("analysis-returns")
            },
        )
        .otherwise(move || {
            when(
                move || picked.get() == 2,
                move || charts::monthly_heat_map(quote).id("analysis-monthly"),
            )
            .otherwise(move || charts::drawdown_chart(quote).id("analysis-drawdown"))
        }),
    ))
    .spacing(8.0)
    .align(HAlign::Leading)
    .grow_w()
}

fn stats_grid(quote: Signal<Load<quotes::Quote>>) -> impl Piece {
    // The day's and the year's high/low are NOT cells here — the range bars above show them
    // with the price positioned between them, which is strictly more information in less
    // space. What remains is what a bar cannot say.
    grid((
        grid_row((
            stat(res::str::stat_open(), quote, |q| format!("{:.2}", q.open)),
            stat(res::str::stat_prev_close(), quote, |q| {
                format!("{:.2}", q.prev_close)
            }),
            stat(res::str::stat_volume(), quote, |q| fmt_volume(q.volume)),
        )),
        grid_row((
            stat(res::str::stat_sma20(), quote, |q| {
                format!("{:.2}", q.sma20())
            }),
            stat(res::str::stat_sma50(), quote, |q| {
                format!("{:.2}", q.sma50())
            }),
            stat(res::str::stat_days(), quote, |q| {
                format!("{}", q.closes.len())
            }),
        )),
    ))
    .spacing(14.0)
    .align(Alignment::TopLeading)
    .grow_w()
    .padding(14.0)
    .background(Color::rgba(0.5, 0.5, 0.5, 0.10))
    .corner_radius(12.0)
}

pub fn detail_page(symbol: &str) -> impl Piece + use<> {
    let quote = quotes::resource_for(symbol).signal();
    let range = quotes::range();
    let symbol = symbol.to_string();

    let header = row((
        column((
            label(move || quote.with(|l| l.ready().map(|q| q.name.clone()).unwrap_or_default()))
                .font(Font::Title2)
                .bold()
                .id("detail-name"),
            label(symbol.clone())
                .font(Font::Footnote)
                .id("detail-symbol"),
        ))
        .spacing(2.0)
        .align(HAlign::Leading)
        .grow_w(),
        column((
            label(move || {
                quote.with(|l| {
                    l.ready()
                        .map(|q| format!("{:.2}", q.last))
                        .unwrap_or_else(|| "…".to_string())
                })
            })
            // A semantic step, not a point size: the hero number has to grow with the reader's
            // accessibility text setting like every other label on the page.
            .font(Font::LargeTitle)
            .bold()
            .id("detail-price"),
            change_chip(quote, "detail-chg".to_string()),
        ))
        .spacing(2.0)
        .align(HAlign::Trailing),
    ))
    .spacing(12.0);

    let ranges: Vec<String> = charts::RANGES.iter().map(|(n, _)| n.to_string()).collect();
    let range_picker = picker(ranges, range).segmented().id("range-picker");
    // The overlay preference is app-wide and persisted, so a two-way binding writes through
    // to prefs rather than living only for this page's lifetime.
    let overlay_on = quotes::overlay();
    Effect::new(move || quotes::persist_overlay(overlay_on.get()));

    scroll(
        column((
            header,
            when(
                move || quote.with(|l| l.is_loading()),
                || {
                    row((
                        spinner(),
                        label(res::str::detail_loading()).font(Font::Callout),
                    ))
                    .spacing(8.0)
                    .id("detail-loading")
                },
            ),
            when(
                move || quote.with(|l| l.error().is_some()),
                move || {
                    label(move || {
                        quote.with(|l| {
                            l.error()
                                .map(|e| res::str::detail_error(e.to_string()).format())
                                .unwrap_or_default()
                        })
                    })
                    .font(Font::Callout)
                    .color(charts::trend_color(false))
                    .id("detail-error")
                },
            ),
            range_picker,
            charts::price_chart(quote),
            // The overlay switch sits under the chart it controls, with the legend naming the
            // two lines by their colors — the chart is otherwise three lines with no key.
            row((
                label(res::str::overlay_label()).font(Font::Callout),
                toggle(overlay_on).id("overlay-toggle"),
                spacer(),
                when(
                    move || overlay_on.get(),
                    || {
                        row((
                            legend(charts::SMA20_COLOR, res::str::overlay_sma20()),
                            legend(charts::SMA50_COLOR, res::str::overlay_sma50()),
                        ))
                        .spacing(12.0)
                        .id("overlay-legend")
                    },
                ),
            ))
            .spacing(10.0)
            .align(VAlign::Center)
            .grow_w(),
            charts::volume_strip(quote),
            analysis_panel(quote),
            // Where the price sits inside today's band and inside the year's — the reading the
            // stats cells below give as bare numbers.
            row((
                range_row(
                    res::str::range_day(),
                    quote,
                    |q| q.low,
                    |q| q.high,
                    "range-day",
                ),
                range_row(
                    res::str::range_52w(),
                    quote,
                    |q| q.week52_low(),
                    |q| q.week52_high(),
                    "range-52w",
                ),
            ))
            .spacing(18.0)
            .grow_w(),
            stats_grid(quote),
            label(res::str::data_attribution()).font(Font::Caption2),
        ))
        .spacing(12.0)
        .align(HAlign::Leading)
        .padding(16.0),
    )
    .grow()
}

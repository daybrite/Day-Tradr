//! One instrument's page: big price header with change chip, the shared range picker, the
//! canvas price chart + volume strip, and a three-column stats grid — every value bound to
//! the symbol's reactive [`Resource`] so a refetch or range tap updates in place.

use crate::charts;
use crate::pages::watchlist::change_chip;
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
) -> AnyPiece {
    column((
        label(title).font(Font::Caption),
        label(move || quote.with(|l| l.ready().map(&value).unwrap_or_default()))
            .font(Font::Callout)
            .bold(),
    ))
    .spacing(1.0)
    .align(HAlign::Leading)
    .grow_w()
    .any()
}

fn stats_grid(quote: Signal<Load<quotes::Quote>>) -> AnyPiece {
    grid((
        grid_row((
            stat(res::str::stat_open(), quote, |q| format!("{:.2}", q.open)),
            stat(res::str::stat_high(), quote, |q| format!("{:.2}", q.high)),
            stat(res::str::stat_low(), quote, |q| format!("{:.2}", q.low)),
        )),
        grid_row((
            stat(res::str::stat_volume(), quote, |q| fmt_volume(q.volume)),
            stat(res::str::stat_52w_high(), quote, |q| {
                format!("{:.2}", q.week52_high())
            }),
            stat(res::str::stat_52w_low(), quote, |q| {
                format!("{:.2}", q.week52_low())
            }),
        )),
        grid_row((
            stat(res::str::stat_prev_close(), quote, |q| {
                format!("{:.2}", q.prev_close)
            }),
            stat(res::str::stat_sma20(), quote, |q| {
                format!("{:.2}", q.sma20())
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

pub fn detail_page(symbol: &str) -> AnyPiece {
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
            .font(Font::System(40.0))
            .bold()
            .id("detail-price"),
            change_chip(quote, "detail-chg".to_string()),
        ))
        .spacing(2.0)
        .align(HAlign::Trailing),
    ))
    .spacing(12.0);

    let ranges: Vec<String> = charts::RANGES.iter().map(|(n, _)| n.to_string()).collect();
    let range_row = picker(ranges, range).segmented().id("range-picker");

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
            range_row,
            charts::price_chart(quote),
            charts::volume_strip(quote),
            stats_grid(quote),
            label(res::str::data_attribution()).font(Font::Caption2),
        ))
        .spacing(12.0)
        .align(HAlign::Leading)
        .padding(16.0),
    )
    .grow()
    .any()
}

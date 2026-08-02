//! The watchlist: one rich card per tracked symbol — name, sparkline, price, and a colored
//! change chip — reconciled by `each` (rows keep their state as the list changes) and tappable
//! through to the symbol's detail page.

use crate::charts;
use crate::quotes;
use crate::res;
use day::prelude::*;

/// "+1.24 (0.62%)" / "−3.10 (1.88%)" — the chip's text, sign folded into the number.
pub fn change_text(q: &quotes::Quote) -> String {
    let sign = if q.change() >= 0.0 { "+" } else { "−" };
    format!(
        "{sign}{:.2} ({:.2}%)",
        q.change().abs(),
        q.change_pct().abs()
    )
}

/// The change chip: white text on the trend color, rounded — the Stocks signature.
pub fn change_chip(quote: Signal<day::reactive::Load<quotes::Quote>>, id: String) -> AnyPiece {
    label(move || quote.with(|l| l.ready().map(change_text).unwrap_or_default()))
        .font(Font::Callout)
        .color(Color::WHITE)
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

fn row_card(symbol: String) -> AnyPiece {
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
        let _ = navigate(&nav_to);
    })
    .id(format!("wl-row-{symbol}"))
}

pub fn watchlist_page() -> AnyPiece {
    let list = quotes::symbols();
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
            each(
                move || list.get(),
                |s| s.clone(),
                |slot| row_card(slot.key()),
            ),
            label(res::str::data_attribution()).font(Font::Caption2),
        ))
        .spacing(10.0)
        .align(HAlign::Leading)
        .padding(16.0),
    )
    .grow()
    .any()
}

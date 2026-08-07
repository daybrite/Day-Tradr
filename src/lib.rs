//! Day Tradr — a stock & commodity quotes app built with [Day](https://daybrite.dev), modeled
//! on Apple Stocks. `root()` is the whole UI, shared by every platform: a sidebar selector
//! whose symbol rows derive reactively from the persisted watchlist (`quotes.rs`), a rich
//! watchlist overview, a canvas-drawn detail chart per instrument, and manage/settings pages.

use day::prelude::*;

mod charts;
mod pages;
mod quotes;

/// Typed constants for the files under `resource/`, generated at build time by `day-build`
/// (§18.5): `res::str::<key>()` for every Fluent message plus the `res::locales` catalog.
pub mod res {
    include!(concat!(env!("OUT_DIR"), "/day_resources.rs"));
}

fn symbol_page(id: &str) -> AnyPiece {
    if quotes::symbols().get_untracked().iter().any(|s| s == id) {
        let page = pages::detail_page(id);
        // Desktop (docs/windows.md): right-click ▸ Open in New Window — a per-symbol
        // Normal window (quote resources and the shared range signal are app-global, so
        // every window tracks the same range — the Stocks-app behavior).
        if capability(Cap::MultiWindow) == Support::Unsupported {
            return page;
        }
        let id = id.to_string();
        page.context_menu(vec![
            menu_item(res::str::open_in_new_window().format()).action(move || {
                let id = id.clone();
                day::open_window(
                    None,
                    day::WindowOptions {
                        title: id.clone(),
                        size: Size::new(720.0, 640.0),
                        min_size: None,
                        app_name: None,
                    },
                    day::WindowKind::Normal,
                    move || symbol_page(&id),
                );
            }),
        ])
    } else {
        // A just-removed key mid-navigation: an empty pane, the selection resets right after.
        spacer().any()
    }
}

pub fn root() -> AnyPiece {
    // Registers every locale under `resource/locales/` (generated, §18.5), then applies the
    // persisted language/theme overrides before the first page builds.
    res::locales::install();
    pages::apply_startup();
    // The Preferences window (docs/windows.md): the settings page as a singleton window on
    // desktop (auto Settings…/⌘, item), the cover fallback on mobile; the sidebar item
    // keeps working everywhere.
    day::register_preferences_with(
        day::WindowOptions {
            title: res::str::nav_settings().format(),
            size: Size::new(560.0, 640.0),
            min_size: None,
            app_name: None,
        },
        pages::settings_page,
    );

    // Start every tracked symbol's fetch now, in the (permanent) root scope.
    let list = quotes::symbols();
    for s in list.get_untracked() {
        let _ = quotes::resource_for(&s);
    }

    let section: Signal<Option<String>> = Signal::new(Some("watchlist".into()));
    selector(section)
        .style(SelectorStyle::Sidebar)
        .title(res::str::app_title())
        .header(sidebar_header)
        .item(
            "watchlist".to_string(),
            res::str::nav_watchlist(),
            pages::watchlist_page,
        )
        // One sidebar row per tracked symbol, re-derived when the watchlist changes; the
        // page for a symbol key comes from `.destination` below.
        .items(move || list.get(), |s: &String| item(s.clone(), s.clone()))
        .destination(|key: &Option<String>| match key {
            Some(id) => symbol_page(id),
            None => spacer().any(),
        })
        .item(
            "manage".to_string(),
            res::str::nav_manage(),
            pages::manage_page,
        )
        .item(
            "settings".to_string(),
            res::str::nav_settings(),
            pages::settings_page,
        )
        .id("nav")
        .any()
}

fn sidebar_header() -> AnyPiece {
    column((
        label(res::str::app_title())
            .font(Font::Headline)
            .id("app-title"),
        label(res::str::app_tagline()).font(Font::Caption),
    ))
    .spacing(2.0)
    .align(HAlign::Leading)
    .padding(12.0)
    .any()
}

// Mobile / embedded entry points — each macro expands to nothing off its own platform.
day::ios_main!("Day Tradr", root);
day::android_main!(root);
day::arkui_main!(root);
day::web_main!("Day Tradr", root);

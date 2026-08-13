//! Day Tradr — a stock & commodity quotes app built with [Day](https://daybrite.dev), modeled
//! on Apple Stocks. `root()` is the whole UI, shared by every platform: a sidebar selector
//! whose symbol rows derive reactively from the persisted watchlist (`quotes.rs`), a rich
//! watchlist overview, a canvas-drawn detail chart per instrument, and manage/settings pages.

use day::prelude::*;
use std::cell::OnceCell;

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
    // The desktop's Add/Remove Symbol items (docs/menus.md). Installed on every target: the
    // toolkits with no menu bar simply have nowhere to draw it, and the phones reach the same
    // add flow through the Symbols tab's `+` button instead.
    pages::install_app_menu();
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

    // Two shells over the same pages (docs/size-classes.md). A phone gets the platform's own
    // top-level idiom — a tab bar — where a sidebar would spend a third of the screen on
    // navigation chrome; anything wider keeps the sidebar, which is what a desktop stocks app
    // looks like. `size_class()` is tracked, so a window dragged across the breakpoint rebuilds
    // into the other shell rather than keeping the one it launched with.
    // The nav id is applied HERE, to whichever shell was chosen — it is what every dayscript
    // waits on, and one call site keeps it honest: tagging both shells would read as a
    // duplicate to `day lint`, which cannot know the two are mutually exclusive.
    let shell = if day::size_class().is_some_and(|c| !c.prefers_split()) {
        tabbed_shell()
    } else {
        sidebar_shell()
    };
    shell.id("nav")
}

/// The phone shell: three tabs, Watchlist first. EACH tab that can reach a symbol carries its own
/// push stack, so a symbol opened from a tab returns to that tab's own root — the standard
/// per-tab-stack behaviour on both platforms.
///
/// The tab keys deliberately match the desktop sidebar's item keys (`watchlist`, `manage`,
/// `settings`), so a route naming a SECTION means the same thing at every size. A symbol does
/// not: the sidebar owns symbol keys at the top level (`MSFT`), while here a symbol is pushed
/// onto the owning tab's stack and its route nests under the tab (`watchlist/MSFT`). Use
/// [`open_symbol`] rather than `navigate` to reach a symbol from a page that serves both.
fn tabbed_shell() -> AnyPiece {
    selector(tab())
        .style(SelectorStyle::Tabs)
        .item_icon(
            "watchlist".to_string(),
            res::str::nav_watchlist(),
            res::vectors::tab_watchlist.clone(),
            watchlist_stack,
        )
        .item_icon(
            "manage".to_string(),
            res::str::nav_symbols(),
            res::vectors::tab_symbols.clone(),
            symbols_stack,
        )
        .item_icon(
            "settings".to_string(),
            res::str::nav_settings(),
            res::vectors::tab_settings.clone(),
            pages::settings_page,
        )
        .any()
}

thread_local! {
    /// The phone shell's state, held outside the build so a page can reach it and so a rebuild
    /// (a size-class morph) keeps the tab and both stacks where the user left them.
    static TAB: OnceCell<Signal<String>> = const { OnceCell::new() };
    static WATCHLIST_PATH: OnceCell<Signal<Vec<String>>> = const { OnceCell::new() };
    static SYMBOLS_PATH: OnceCell<Signal<Vec<String>>> = const { OnceCell::new() };
}

/// Tabs always have a selection, so the signal is a plain key, not an `Option`.
fn tab() -> Signal<String> {
    TAB.with(|c| *c.get_or_init(|| Scope::detached().enter(|| Signal::new("watchlist".into()))))
}

fn watchlist_path() -> Signal<Vec<String>> {
    WATCHLIST_PATH.with(|c| *c.get_or_init(|| Scope::detached().enter(|| Signal::new(Vec::new()))))
}

fn symbols_path() -> Signal<Vec<String>> {
    SYMBOLS_PATH.with(|c| *c.get_or_init(|| Scope::detached().enter(|| Signal::new(Vec::new()))))
}

/// Open a symbol's detail page, whichever shell is live.
///
/// The sidebar shell owns symbol keys as top-level routes, so `navigate` claims them. A STACK
/// does not: its `push` refuses every non-empty key by design, because a stack is driven by its
/// path rather than by route strings (day-pieces/src/nav.rs). A row that only called `navigate`
/// therefore did nothing at all on a phone — the tap registered and no page opened.
pub fn open_symbol(symbol: &str) {
    if navigate(symbol) {
        return;
    }
    let path = if tab().get_untracked() == "manage" {
        symbols_path()
    } else {
        watchlist_path()
    };
    // Guard the repeat: tapping the row of the symbol already on top would stack a duplicate
    // page, and the back button would then land on the same detail again.
    path.update(|p| {
        if p.last().map(String::as_str) != Some(symbol) {
            p.push(symbol.to_string());
        }
    });
}

/// The Watchlist tab: the watchlist as the stack's root, a row pushing that symbol's detail page.
///
/// The stack is what makes a row tappable on a phone at all. A watchlist row navigates by calling
/// `navigate(symbol)` (pages/watchlist.rs), which needs a surface willing to accept the symbol as
/// a route; with the page mounted bare in the tab there was none, so tapping a row did nothing.
fn watchlist_stack() -> AnyPiece {
    stack(watchlist_path(), pages::watchlist_page())
        .title(res::str::nav_watchlist())
        .destination(|key: &String| symbol_page(key))
        .id("watchlist-stack")
        .any()
}

/// The Symbols tab: the editable list as the stack's root, each row pushing that symbol's
/// detail page, and a `+` in the navigation bar for adding one.
fn symbols_stack() -> AnyPiece {
    stack(symbols_path(), pages::manage_page())
        .title(res::str::nav_symbols())
        // The nav bar's trailing button (docs/navigation.md) — the phones have no window
        // toolbar to put this in.
        .bar_action(
            res::vectors::add_symbol.clone(),
            res::str::menu_add_symbol(),
            pages::prompt_for_symbol,
        )
        .destination(|key: &String| symbol_page(key))
        .id("symbols-stack")
        .any()
}

/// The desktop shell: the sidebar this app has always had, with one row per tracked symbol.
fn sidebar_shell() -> AnyPiece {
    let list = quotes::symbols();
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
            res::str::nav_symbols(),
            pages::manage_page,
        )
        .item(
            "settings".to_string(),
            res::str::nav_settings(),
            pages::settings_page,
        )
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
day::macos_main!("Day Tradr", root);
day::android_main!(root);
day::arkui_main!(root);
day::web_main!("Day Tradr", root);

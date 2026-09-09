//! Day Tradr — a stock & commodity quotes app built with [Day](https://daybrite.dev), modeled
//! on Apple Stocks. `root()` is the whole UI, shared by every platform: a sidebar nav
//! whose symbol rows derive reactively from the persisted watchlist (`quotes.rs`), a rich
//! watchlist overview, a canvas-drawn detail chart per instrument, and manage/settings pages.

use day::prelude::*;

mod charts;
mod pages;
mod quotes;

// The mobile / embedded entry point. Expands to the export each platform's shell binds
// against — and to nothing at all on a plain cargo desktop build, where src/main.rs is the
// entry instead.
day::day_start!("Day Tradr", root);

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
            return page.any();
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
                        ..Default::default()
                    },
                    day::WindowKind::Normal,
                    move || symbol_page(&id),
                );
            }),
        ])
        .any()
    } else {
        // A just-removed key mid-navigation: an empty pane, the selection resets right after.
        spacer().any()
    }
}

pub fn root() -> impl Piece {
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
            ..Default::default()
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
    // File ▸ New Window (docs/windows.md): the SAME shell again, which is why the tab, the two
    // push stacks and the chart range live on a `Scene` — each window gets its own.
    day::register_new_window(window_shell);

    window_shell()
}

/// One window's UI — the first window's, and every File ▸ New Window's.
fn window_shell() -> impl Piece {
    Scene::scoped(|_scene| {
        // Two shells, two types: `Either` picks one without boxing either.
        let shell = if day::size_class().is_some_and(|c| !c.prefers_split()) {
            Either::Left(tabbed_shell())
        } else {
            Either::Right(sidebar_shell())
        };
        shell.id("nav")
    })
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
fn tabbed_shell() -> impl Piece {
    nav(tab())
        .style(NavStyle::Tabs)
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
}

/// Everything ONE WINDOW owns (docs/state.md): which tab it is on, each tab's push stack, and
/// the chart range it is showing. The watchlist itself and every persisted preference are
/// app-wide (`quotes::Watchlist`) — the list of symbols you track is the same list in every
/// window; where you are in it is not.
///
/// Held outside the piece build (in the window's scope, via `Ambient::scoped`) so a page can
/// reach it and so a rebuild — a size-class morph between the tabbed and sidebar shells — keeps
/// the tab and both stacks where the user left them.
#[derive(Clone, Copy)]
pub(crate) struct Scene {
    /// Tabs always have a selection, so the key is plain, not an `Option`.
    tab: Signal<String>,
    watchlist_path: Signal<Vec<String>>,
    symbols_path: Signal<Vec<String>>,
    pub(crate) range: Signal<usize>,
}

impl Ambient for Scene {
    fn create() -> Self {
        Scene {
            tab: Signal::new("watchlist".into()),
            watchlist_path: Signal::new(Vec::new()),
            symbols_path: Signal::new(Vec::new()),
            range: Signal::new(3), // 1Y
        }
    }
}

/// This window's `Scene`.
///
/// Two resolutions, because there are two moments a page reaches for it. While a piece BUILDS,
/// the ambient one is this window's. Later — a row tap, a menu item, anything that runs from a
/// handler — there is no build scope to read from, and the window the user is looking at is the
/// one the command means: that is `focused()` (docs/state.md).
pub(crate) fn scene() -> Scene {
    Scene::try_ambient()
        .or_else(Scene::focused)
        .expect("no window is open, so there is no Scene to act on")
}

fn tab() -> Signal<String> {
    scene().tab
}

fn watchlist_path() -> Signal<Vec<String>> {
    scene().watchlist_path
}

fn symbols_path() -> Signal<Vec<String>> {
    scene().symbols_path
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
fn watchlist_stack() -> impl Piece {
    nav_stack(watchlist_path(), pages::watchlist_page())
        .title(res::str::nav_watchlist())
        .destination(|key: &String| symbol_page(key))
        .id("watchlist-stack")
}

/// The Symbols tab: the editable list as the stack's root, each row pushing that symbol's
/// detail page, and a `+` in the navigation bar for adding one.
fn symbols_stack() -> impl Piece {
    nav_stack(symbols_path(), pages::manage_page())
        .title(res::str::nav_symbols())
        // Adding acts on the LIST this stack's root shows, so it rides the root page's chrome
        // (docs/toolbars.md) and is gone from the symbol pages pushed over it.
        .toolbar(
            toolbar_button("tb-add-symbol", res::str::menu_add_symbol())
                .image(res::vectors::add_symbol.clone())
                .placement(ToolbarPlacement::Primary)
                .action(pages::prompt_for_symbol),
        )
        .destination(|key: &String| symbol_page(key))
        .id("symbols-stack")
}

/// The desktop shell: the sidebar this app has always had, with one row per tracked symbol.
fn sidebar_shell() -> impl Piece {
    let list = quotes::symbols();
    let section: Signal<Option<String>> = Signal::new(Some("watchlist".into()));
    nav(section)
        .style(NavStyle::Sidebar)
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
}

fn sidebar_header() -> impl Piece {
    column((
        label(res::str::app_title())
            .font(Font::Headline)
            .id("app-title"),
        label(res::str::app_tagline()).font(Font::Caption),
    ))
    .spacing(2.0)
    .align(HAlign::Leading)
    .padding(12.0)
}

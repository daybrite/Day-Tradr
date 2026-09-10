# Day Tradr — UI strings (https://daybrite.dev/docs/localization). Add a locale by dropping a
# sibling folder (e.g. locales/fr/app.ftl) and translating — the generated
# res::locales::install() in src/lib.rs picks up every locale directory by itself.

language_name = English
app_title = Day Tradr
app_tagline = Markets

nav_watchlist = Watchlist
nav_settings = Settings

watchlist_title = Watchlist
data_mock = Mock data · deterministic fixtures
data_live = Live data
data_attribution = Data by Yahoo Finance — free end-of-day quotes.

detail_loading = Loading quotes…
detail_error = Could not load: { $error }

stat_open = Open
stat_volume = Volume
stat_prev_close = Prev Close
stat_sma20 = SMA 20
stat_sma50 = SMA 50
stat_days = Sessions

manage_list_section = Tracked symbols
manage_add_section = Add a symbol
manage_symbol_label = Ticker
manage_symbol_hint = Yahoo ticker — AAPL, SPY, GC=F, EURUSD=X. Case does not matter.
manage_add = Add
manage_preset_label = Suggestions
manage_add_preset = Add selected
manage_remove = Remove
manage_status_added = Added
manage_status_removed = Removed
manage_status_exists = Already tracked
manage_status_empty = Type a ticker first

settings_about_section = About
settings_name_label = Name
settings_version_label = Version
settings_build_label = Built
settings_website = Built with Day — daybrite.dev
settings_data_link = Data source — Yahoo Finance
settings_language_section = Language
settings_language_label = Language
settings_system = System
settings_theme_section = Appearance
settings_theme_label = Theme
theme_light = Light
theme_dark = Dark
settings_data_section = Data
settings_refresh_hint = Refetch every tracked symbol from the source.
settings_refresh = Refresh all

open_in_new_window = Open in New Window

# Watchlist summary + ordering (added with the breadth strip and sort picker)
breadth_up = Advancing
breadth_down = Declining
breadth_best = Best { $pct }
breadth_worst = Worst { $pct }
sort_manual = Custom
sort_name = Name
sort_change = Change
watchlist_empty = No symbols tracked
watchlist_empty_hint = Add one from the Symbols page.

# Detail chart overlays + range bars
overlay_label = Averages
overlay_sma20 = 20-day
overlay_sma50 = 50-day
range_day = Day range
range_52w = 52-week range
chip_both = Change + %
chip_percent = Percent
chip_absolute = Change

# Tabbed mobile shell + the add flow reached from the nav bar's + button
nav_symbols = Symbols
add_symbol_title = Add a symbol
add_symbol_body = Enter a Yahoo ticker — AAPL, SPY, GC=F.
menu_add_symbol = Add Symbol…
menu_remove_symbol = Remove Symbol
menu_symbols = Symbols

# Network proxy for quote fetches (docs: quotes.rs `proxied`)
settings_proxy_section = Network
settings_proxy_label = Proxy
settings_proxy_hint = Quote requests go through this. Use %u where the whole Yahoo URL should be inserted, or %p where only its path should go; a template with neither is treated as a prefix. Leave empty to fetch directly — the web build needs a proxy because browsers block cross-site requests to Yahoo.
settings_proxy_apply = Apply
settings_proxy_relay = Use Daybrite relay
settings_proxy_direct = Fetch directly

# The analysis panel under the detail chart, and the watchlist's performance card
analysis_label = Analysis
analysis_drawdown = Drawdown
analysis_returns = Daily moves
analysis_monthly = Monthly
axis_return = One-session move
performance_title = Performance, indexed to 100

# New interactive charts (day-piece-charts README "Selection").
analysis_profile = Volume profile
wl_risk_title = Risk & return
wl_corr_title = Correlation
wl_risk_none = Hover or tap a symbol
wl_corr_none = Hover or tap a cell
wl_risk_readout = { $symbol }: { $ret } return at { $vol } volatility

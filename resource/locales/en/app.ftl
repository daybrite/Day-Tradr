# Day Tradr — UI strings (https://daybrite.dev/docs/localization). Add a locale by dropping a
# sibling folder (e.g. locales/fr/app.ftl) and translating — the generated
# res::locales::install() in src/lib.rs picks up every locale directory by itself.

language_name = English
app_title = Day Tradr
app_tagline = Markets

nav_watchlist = Watchlist
nav_manage = Symbols
nav_settings = Settings

watchlist_title = Watchlist
data_mock = Mock data · deterministic fixtures
data_live = Live data
data_attribution = Data by stooq.com — free end-of-day quotes.

detail_loading = Loading quotes…
detail_error = Could not load: { $error }

stat_open = Open
stat_high = High
stat_low = Low
stat_volume = Volume
stat_52w_high = 52W High
stat_52w_low = 52W Low
stat_prev_close = Prev Close
stat_sma20 = SMA 20
stat_days = Sessions

manage_list_section = Tracked symbols
manage_add_section = Add a symbol
manage_symbol_label = Ticker
manage_symbol_hint = stooq format — AAPL.US, XAUUSD, CL.F. Case does not matter.
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
settings_data_link = Data source — stooq.com
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

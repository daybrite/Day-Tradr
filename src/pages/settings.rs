//! Settings: About (name, version, build date, links), language and appearance overrides
//! (persisted via day-part-prefs; the Day-Skies pattern), and the data refresh action.

use crate::quotes;
use crate::res;
use day::prelude::*;

const PREF_LOCALE: &str = "tradr.locale"; // a res::locales::ALL tag; absent = system
const PREF_THEME: &str = "tradr.theme"; // "light" | "dark"; absent = system

/// Apply the persisted language and theme overrides — called once from `root()`, right after
/// the locale catalog installs and before the first page builds. The shared piece owns the
/// mechanics (docs/windows.md), including the env-wins rule: a `DAY_THEME`/`--locale` launch
/// keeps its override no matter what an earlier run persisted.
pub fn apply_startup() {
    day_piece_settings::apply_startup(PREF_THEME, PREF_LOCALE);
}

pub fn settings_page() -> AnyPiece {
    let mut parts: Vec<AnyPiece> = vec![
        AnyPiece::new(
            section((
                labeled(
                    res::str::settings_name_label(),
                    label(res::str::app_title()),
                ),
                labeled(
                    res::str::settings_version_label(),
                    label(env!("CARGO_PKG_VERSION")).id("about-version"),
                ),
                labeled(
                    res::str::settings_build_label(),
                    label(env!("DAY_TRADR_BUILD_DATE")).id("about-build"),
                ),
                link(res::str::settings_website(), "https://daybrite.dev").id("about-day"),
                link(res::str::settings_data_link(), "https://stooq.com").id("about-stooq"),
            ))
            .title(res::str::settings_about_section()),
        ),
        // Language + appearance: the shared settings rows (docs/windows.md —
        // day-piece-settings). Same ids, same keys, same live apply; the appearance row is
        // Cap::Appearance-gated inside the piece.
        AnyPiece::new(
            section((day_piece_settings::language_picker(PREF_LOCALE, res::locales::ALL),))
                .title(res::str::settings_language_section()),
        ),
    ];
    if capability(Cap::Appearance) != Support::Unsupported {
        parts.push(AnyPiece::new(
            section((day_piece_settings::appearance_picker(PREF_THEME),))
                .title(res::str::settings_theme_section()),
        ));
    }
    parts.push(AnyPiece::new(
        section((
            label(res::str::settings_refresh_hint()).font(Font::Footnote),
            button(res::str::settings_refresh())
                .action(quotes::reload_all)
                .prominent()
                .id("refresh-all"),
        ))
        .title(res::str::settings_data_section()),
    ));

    scroll(
        column((form(PieceVec(parts)),))
            .spacing(12.0)
            .align(HAlign::Leading)
            .padding(16.0),
    )
    .grow()
    .any()
}

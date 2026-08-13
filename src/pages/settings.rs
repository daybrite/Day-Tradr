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
                link(res::str::settings_data_link(), "https://finance.yahoo.com")
                    .id("about-source"),
            ))
            .title(res::str::settings_about_section()),
        ),
        // Language + appearance: the shared settings rows (docs/windows.md —
        // day-piece-settings). Same ids, same keys, same live apply; the appearance row is
        // Cap::Appearance-gated inside the piece.
        AnyPiece::new(
            section((day_piece_settings::language_picker(
                PREF_LOCALE,
                res::locales::ALL,
            ),))
            .title(res::str::settings_language_section()),
        ),
    ];
    if capability(Cap::Appearance) != Support::Unsupported {
        parts.push(AnyPiece::new(
            section((day_piece_settings::appearance_picker(PREF_THEME),))
                .title(res::str::settings_theme_section()),
        ));
    }
    // The proxy every quote fetch is routed through. Edited as a template rather than a
    // host, because the two proxy families take the target differently: a placeholder in a
    // query parameter, or a prefix the target is appended to (quotes::proxied).
    let proxy = quotes::proxy();
    let entry = Signal::new(proxy.get_untracked());
    parts.push(AnyPiece::new(
        section((
            labeled(
                res::str::settings_proxy_label(),
                text_field(entry)
                    .placeholder(quotes::DEFAULT_WEB_PROXY.to_string())
                    .id("proxy-field"),
            ),
            label(res::str::settings_proxy_hint()).font(Font::Footnote),
            row((
                button(res::str::settings_proxy_apply())
                    .action(move || {
                        quotes::set_proxy(&entry.get_untracked());
                        // Re-fetch every symbol through the new route, so the field's effect is
                        // visible immediately rather than at the next refresh.
                        quotes::reload_all();
                    })
                    .prominent()
                    .id("proxy-apply"),
                // Fills the field rather than applying, so the template is visible and editable
                // before it takes effect — the relay's url is long enough that nobody should
                // have to type it on a phone.
                button(res::str::settings_proxy_relay())
                    .action(move || entry.set(quotes::DAYBRITE_WEB_PROXY.to_string()))
                    .id("proxy-relay"),
                button(res::str::settings_proxy_direct())
                    .action(move || {
                        entry.set(String::new());
                        quotes::set_proxy("");
                        quotes::reload_all();
                    })
                    .id("proxy-direct"),
            ))
            .spacing(8.0),
        ))
        .title(res::str::settings_proxy_section()),
    ));
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

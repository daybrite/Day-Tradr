//! Manage the watchlist: add a symbol by ticker (Yahoo Finance format, e.g. `AAPL`, `GC=F`),
//! add from a picker of suggestions, and remove tracked rows. Mirrors the Apple Stocks
//! edit sheet as a plain form page.

use crate::quotes;
use crate::res;
use day::prelude::*;

#[derive(Clone, PartialEq)]
enum Status {
    Idle,
    Added,
    Removed,
    Exists,
    Empty,
}

impl Status {
    fn text(&self) -> String {
        match self {
            Status::Idle => String::new(),
            Status::Added => res::str::manage_status_added().format(),
            Status::Removed => res::str::manage_status_removed().format(),
            Status::Exists => res::str::manage_status_exists().format(),
            Status::Empty => res::str::manage_status_empty().format(),
        }
    }
}

pub fn manage_page() -> AnyPiece {
    let sym_list = quotes::symbols();
    let entry = Signal::new(String::new());
    let status = Signal::new(Status::Idle);
    let preset_ix = Signal::new(0usize);

    let add_typed = move || {
        let symbol = quotes::normalize(&entry.get_untracked());
        if symbol.is_empty() {
            status.set(Status::Empty);
            return;
        }
        status.set(if quotes::add(&symbol) {
            entry.set(String::new());
            Status::Added
        } else {
            Status::Exists
        });
    };

    // One recycling-list row per tracked symbol (drag to reorder — the order IS the sidebar
    // order, persisted with the list).
    let rows = list(
        move || sym_list.get(),
        |s: &String| s.clone(),
        move |slot| {
            // Recycling rows (docs/list.md): cells rebind as the list changes or reorders, so
            // the action reads the slot's CURRENT key at click time and the id re-registers
            // reactively (`id_of`) — a build-time key would go stale.
            row((
                column((
                    label(move || slot.get()).font(Font::Headline),
                    label(move || quotes::preset_name(&slot.get()).to_string())
                        .font(Font::Footnote),
                ))
                .spacing(1.0)
                .align(HAlign::Leading)
                .grow_w(),
                button(res::str::manage_remove())
                    .action(move || {
                        quotes::remove(&slot.key());
                        status.set(Status::Removed);
                    })
                    .id_of(move || format!("sym-remove-{}", slot.get())),
            ))
            .spacing(8.0)
            .padding(Insets::symmetric(4.0, 0.0))
        },
    )
    .row_height(RowHeight::Uniform(52.0))
    .reorderable(true)
    .on_reorder(quotes::move_symbol)
    .id("sym-rows")
    .height(312.0);

    let preset_names: Vec<String> = quotes::PRESETS
        .iter()
        .map(|s| format!("{s} — {}", quotes::preset_name(s)))
        .collect();

    scroll(
        column((
            form((
                section((rows,)).title(res::str::manage_list_section()),
                section((
                    labeled(
                        res::str::manage_symbol_label(),
                        text_field(entry)
                            .placeholder("AAPL".to_string())
                            .id("sym-field"),
                    ),
                    label(res::str::manage_symbol_hint()).font(Font::Footnote),
                    button(res::str::manage_add())
                        .action(add_typed)
                        .prominent()
                        .id("sym-add"),
                    labeled(
                        res::str::manage_preset_label(),
                        picker(preset_names, preset_ix).id("preset-picker"),
                    ),
                    button(res::str::manage_add_preset())
                        .action(move || {
                            let sym = quotes::PRESETS
                                [preset_ix.get_untracked().min(quotes::PRESETS.len() - 1)];
                            status.set(if quotes::add(sym) {
                                Status::Added
                            } else {
                                Status::Exists
                            });
                        })
                        .id("preset-add"),
                ))
                .title(res::str::manage_add_section()),
            )),
            label(move || status.get().text())
                .font(Font::Footnote)
                .id("sym-status"),
        ))
        .spacing(12.0)
        .align(HAlign::Leading)
        .padding(16.0),
    )
    .grow()
    .any()
}

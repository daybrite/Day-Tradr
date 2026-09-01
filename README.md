# Day Tradr

A [Day](https://daybrite.dev) app: one Rust codebase, native widgets on every platform.

## Run it

`day launch --git` clones this repo, builds it for your desktop, and runs it — no checkout needed:

```sh
cargo install day-cli
day doctor                                                 # what's installed, what's missing
day launch --git https://github.com/daybrite/Day-Tradr.git
```

`day doctor` prints the fix for anything it can't find. `day launch --git` prints where it put the
checkout, so you can `cd` there and edit the code.

From inside a clone, name a target instead. Day compiles **exactly one backend per binary**, and
the Day CLI supplies the right feature for each:

```sh
day launch -p windows-xaml   # build + run
day build  -p windows-xaml   # build only
```

Targets live in `Day.toml`. A bare `cargo build` uses this crate's default `mock` backend, which is
what lets rust-analyzer and `cargo check` work with no flags. To pick a real one from plain cargo,
turn the default off as well — otherwise `mock` and your choice are both on, which is two backends
and a compile error:

```sh
cargo build --no-default-features --features appkit    # or gtk / qt / uikit / mdc / xaml / dom
```

## What's inside

- `src/lib.rs` — the UI (`root()`), shared across every platform: a sidebar selector
  ([navigation](https://daybrite.dev/docs/navigation)) with one row per tracked symbol, plus
  a Preferences window that becomes a fullscreen cover on mobile.
- `src/quotes.rs` — the data layer: the persisted watchlist, Yahoo Finance fetches, and one
  memoized reactive [`Resource`](https://daybrite.dev/docs/async) per symbol. `TRADR_MOCK=1`
  swaps in a deterministic generated series so screenshots and asserts are reproducible.
- `src/charts.rs` — every canvas: the price chart with moving-average overlays, the volume
  strip, the watchlist sparklines, and the low→high range bars
  ([shapes](https://daybrite.dev/docs/shapes)).
- `src/pages/watchlist.rs` — the rows, the breadth summary, the sort control, and the
  tap-to-cycle change chip.
- `src/pages/detail.rs` — one instrument: price header, range picker, chart + overlays,
  range bars, stats.
- `src/pages/manage.rs` — add by ticker or preset, remove, drag to reorder.
- `resource/locales/en/app.ftl` — every user-facing string ([localization](https://daybrite.dev/docs/localization));
  `fr`, `ar` (RTL) and `zh-CN` sit beside it.
- `dayscript/walkthrough.yaml` — a [dayscript](https://daybrite.dev/docs/dayscript) UI test
  over the whole app: `day launch -p macos-appkit --script dayscript/walkthrough.yaml --env TRADR_MOCK=1`.
- `platform/` — the thin native host projects (Xcode / Gradle / hvigor) the mobile targets
  build through; `day build` keeps their identity in sync with `Day.toml`.
- `Day.toml` — app metadata + the target list.

`day lint` checks routes, element ids, and locale coverage.

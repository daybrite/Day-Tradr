# Day Tradr

A stock watchlist that opens on the day at a glance, built with [Day](https://daybrite.dev) in one
Rust codebase and rendered with the platform's own widgets on iPhone, Android, Mac, Windows, Linux,
HarmonyOS, and the web.

<p align="center">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/ios-uikit/en/watchlist.png" width="200" alt="The watchlist on iPhone">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/android-mdc/en/detail.png" width="200" alt="A symbol's detail on Android">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/ios-uikit/en/detail-1m.png" width="200" alt="One month of a symbol on iPhone">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/android-mdc/en/manage.png" width="200" alt="Managing the watchlist on Android">
</p>

## Run it in one command

Install the `day` CLI, then let it clone, build, and launch the app for your desktop:

```sh
cargo install day-cli
day launch --git https://github.com/daybrite/Day-Tradr.git
```

`day doctor` lists what your platform's toolkit needs and prints the install command for anything
missing. The launch prints where it put the checkout, so you can open the code and change it.

## What you get

The list opens on a summary of the day: how many of your symbols are up, how many are down, and
the best and worst movers by name. Every symbol gets a card with a sparkline of recent movement,
the price, and a change chip. Tap the chip to cycle it between the change, the percentage, or
both, and sort the list by your own order, by name, or by today's move.

<p align="center">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/macos-appkit/en/detail.png" width="720" alt="A symbol's detail on macOS, with the watchlist in the sidebar">
</p>

Tap a symbol for the full picture: a price chart with 20-day and 50-day moving averages you can
switch off, a volume strip beneath it, and range bars that show where the price sits inside
today's high and low and inside the year's. Five ranges run from a month to everything the
source has.

- Stocks, ETFs, commodity futures, and currency pairs, added by ticker or picked from suggestions.
- Reorder the watchlist by dragging and remove a symbol by swiping its row away.
- English, French, Arabic, and Simplified Chinese, with right-to-left layout in Arabic.
- Light and dark, following the system or set by hand.

Quotes are Yahoo Finance's free end-of-day data, so this is a watchlist rather than a trading
terminal. There is no account and no brokerage connection.

## The same code on every platform

These captures come from the app's own CI, which runs the walkthrough on every target and
publishes the results to the [gallery](https://daybrite.dev/gallery/Day-Tradr/).

| macOS · AppKit | Windows · XAML | Linux · GTK |
|:---:|:---:|:---:|
| <img src="https://daybrite.github.io/Day-Tradr/gallery/macos-appkit/en/watchlist.png" width="300" alt="Watchlist on macOS"> | <img src="https://daybrite.github.io/Day-Tradr/gallery/windows-xaml/en/watchlist.png" width="300" alt="Watchlist on Windows"> | <img src="https://daybrite.github.io/Day-Tradr/gallery/linux-gtk/en/watchlist.png" width="300" alt="Watchlist on GTK"> |

| Linux · Qt | Web · DOM | HarmonyOS · ArkUI |
|:---:|:---:|:---:|
| <img src="https://daybrite.github.io/Day-Tradr/gallery/linux-qt/en/detail.png" width="300" alt="Detail on Qt"> | <img src="https://daybrite.github.io/Day-Tradr/gallery/web-dom/en/detail.png" width="300" alt="Detail in the browser"> | <img src="https://daybrite.github.io/Day-Tradr/gallery/harmony-arkui/en/detail.png" width="150" alt="Detail on HarmonyOS"> |

The sorted list, the absolute-change chips, and the Arabic layout:

<p align="center">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/ios-uikit/en/watchlist-sorted.png" width="200" alt="Watchlist sorted by today's move, on iPhone">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/android-mdc/en/watchlist-chip-absolute.png" width="200" alt="Absolute-change chips on Android">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/ios-uikit/ar/watchlist.png" width="200" alt="The watchlist in Arabic on iPhone">
  <img src="https://daybrite.github.io/Day-Tradr/gallery/android-mdc/fr/settings.png" width="200" alt="Settings in French on Android">
</p>

## Build from a clone

Day compiles one toolkit backend per binary, so name a target when you build or launch. Every
target the app ships is listed in `Day.toml`.

```sh
day doctor                       # toolchains present and missing, with fixes
day launch -p macos-appkit       # build + run
day launch -p windows-xaml       # on a Windows host
day launch -p ios-uikit          # needs a booted Simulator
day launch -p android-mdc        # needs a JDK and a running emulator or device
day launch -p web-dom            # serves the WebAssembly build locally
```

A bare `cargo build` uses the crate's default `mock` backend, which is what lets rust-analyzer and
`cargo check` work with no flags. To pick a toolkit from plain cargo, turn the default off first:

```sh
cargo build --no-default-features --features appkit    # or gtk / qt / uikit / mdc / xaml / dom
```

Set `TRADR_MOCK=1` to replace the Yahoo Finance fetch with a deterministic generated series, so
screenshots and assertions come out the same every run:

```sh
day launch -p macos-appkit --env TRADR_MOCK=1 --script dayscript/walkthrough.yaml
```

That [dayscript](https://daybrite.dev/docs/dayscript) walks the whole app, and it is the script CI
runs on every target and locale to produce the gallery.

To build against a local `day` checkout instead of the pinned git revision, let the CLI write and
verify the patch table:

```sh
day patch --local /path/to/day
```

## Inside the code

- `src/lib.rs` is `root()`: a sidebar `selector` with one row per tracked symbol, plus a
  Preferences window that becomes a fullscreen cover on mobile.
- `src/quotes.rs` is the data layer: the persisted watchlist, the Yahoo Finance fetch, and one
  memoized reactive [`Resource`](https://daybrite.dev/docs/internal/async) per symbol.
- `src/charts.rs` draws every canvas: the price chart with its overlays, the volume strip, the
  sparklines, and the range bars ([shapes](https://daybrite.dev/docs/internal/shapes)).
- `src/pages/watchlist.rs`, `detail.rs`, and `manage.rs` are the three screens.
- `resource/locales/` carries the Fluent strings for `en`, `fr`, `ar`, and `zh-CN`.
- `platform/` holds the thin native host projects the mobile targets build through.

`day lint` checks routes, element ids, and locale coverage.

Data by Yahoo Finance. Day Tradr is open source under the Apache-2.0 license.

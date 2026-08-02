//! Canvas drawing for Day Tradr: the detail page's price chart + volume strip, and the
//! watchlist's sparklines (https://daybrite.dev/docs/shapes — §11 canvas). Every closure here
//! is a reactive display list: it reads the quote/range signals, so a range tap or a refetch
//! re-records and the backend replays natively.

use day::prelude::*;

/// The range picker's slices, in trading days (`usize::MAX` = everything the source has).
pub const RANGES: [(&str, usize); 5] = [
    ("1M", 22),
    ("3M", 66),
    ("6M", 132),
    ("1Y", 252),
    ("All", usize::MAX),
];

/// `c` at opacity `a` — chart fills reuse the trend color at several alphas.
fn faded(c: Color, a: f64) -> Color {
    Color::rgba(c.r, c.g, c.b, a)
}

/// Price up/flat vs the window's first close → the Stocks green; down → the Stocks red.
pub fn trend_color(up: bool) -> Color {
    if up {
        Color::hex(0x34C759)
    } else {
        Color::hex(0xFF3B30)
    }
}

fn grid_color(dark: bool) -> Color {
    if dark {
        Color::rgba(1.0, 1.0, 1.0, 0.12)
    } else {
        Color::rgba(0.0, 0.0, 0.0, 0.10)
    }
}

fn axis_text(dark: bool) -> Color {
    if dark {
        Color::rgba(1.0, 1.0, 1.0, 0.55)
    } else {
        Color::rgba(0.0, 0.0, 0.0, 0.45)
    }
}

/// Round a raw interval up to a "nice" 1/2/5×10ⁿ step for the horizontal gridlines.
fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let n = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    n * mag
}

/// Map a series slice into x/y points within `rect` (y inverted: larger price = higher).
fn project(closes: &[f64], min: f64, max: f64, rect: Rect) -> Vec<Point> {
    let n = closes.len();
    let span = (max - min).max(1e-9);
    closes
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let fx = if n <= 1 {
                1.0
            } else {
                i as f64 / (n - 1) as f64
            };
            Point::new(
                rect.origin.x + fx * rect.size.width,
                rect.origin.y + (1.0 - (c - min) / span) * rect.size.height,
            )
        })
        .collect()
}

fn polyline(d: &mut day::prelude::Draw, pts: &[Point], color: Color, width: f64) {
    for pair in pts.windows(2) {
        d.stroke(Shape::Line(pair[0], pair[1]), color, width);
    }
}

/// The big price chart: gridlines with right-edge price labels, a gradient area fill under
/// the price line, the line itself in trend color, a dashed reference line at the window's
/// first close, first/last date labels, and a halo dot on the latest price.
pub fn price_chart(quote: Signal<day::reactive::Load<crate::quotes::Quote>>) -> AnyPiece {
    let range = crate::quotes::range();
    canvas(move |d, size| {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return;
        };
        let dark = day::dark_mode();
        let (_, days) = RANGES[range.get().min(RANGES.len() - 1)];
        let closes = crate::quotes::tail(&q.closes, days);
        if closes.len() < 2 || size.width < 40.0 {
            return;
        }
        let dates = &q.dates[q.dates.len() - closes.len()..];
        // Chart body inset: room for the price labels on the right and dates below.
        let inset = Rect::new(0.0, 8.0, (size.width - 56.0).max(10.0), size.height - 34.0);
        let (mut min, mut max) = closes
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let pad = ((max - min) * 0.08).max(max * 0.002);
        min -= pad;
        max += pad;

        // Horizontal gridlines on nice price steps, labeled at the right edge.
        let step = nice_step((max - min) / 4.0);
        let mut gy = (min / step).ceil() * step;
        while gy < max {
            let y = inset.origin.y + (1.0 - (gy - min) / (max - min)) * inset.size.height;
            d.stroke(
                Shape::Line(
                    Point::new(inset.origin.x, y),
                    Point::new(inset.origin.x + inset.size.width, y),
                ),
                grid_color(dark),
                1.0,
            );
            d.text(
                &format!("{gy:.2}"),
                Point::new(inset.origin.x + inset.size.width + 6.0, y - 6.0),
                day::prelude::TextStyle {
                    size: 10.0,
                    color: axis_text(dark),
                    anchor: TextAnchor::Leading,
                },
            );
            gy += step;
        }

        let pts = project(closes, min, max, inset);
        let first = closes[0];
        let up = *closes.last().unwrap_or(&first) >= first;
        let line = trend_color(up);

        // Gradient area under the line: the polyline closed down to the baseline.
        let mut area = pts.clone();
        area.push(Point::new(
            inset.origin.x + inset.size.width,
            inset.origin.y + inset.size.height,
        ));
        area.push(Point::new(
            inset.origin.x,
            inset.origin.y + inset.size.height,
        ));
        d.fill(
            Shape::Polygon(area),
            LinearGradient::vertical(faded(line, 0.30), faded(line, 0.02)),
        );

        // Dashed reference at the window's first close.
        let ref_y = inset.origin.y + (1.0 - (first - min) / (max - min)) * inset.size.height;
        let dash = 5.0;
        let mut x = inset.origin.x;
        while x < inset.origin.x + inset.size.width {
            d.stroke(
                Shape::Line(
                    Point::new(x, ref_y),
                    Point::new((x + dash).min(inset.origin.x + inset.size.width), ref_y),
                ),
                Color::rgba(0.5, 0.5, 0.5, 0.55),
                1.0,
            );
            x += dash * 2.0;
        }

        polyline(d, &pts, line, 2.0);

        // Latest price: a soft halo + solid dot.
        if let Some(p) = pts.last() {
            d.fill(
                Shape::Ellipse(Rect::new(p.x - 7.0, p.y - 7.0, 14.0, 14.0)),
                faded(line, 0.25),
            );
            d.fill(
                Shape::Ellipse(Rect::new(p.x - 3.5, p.y - 3.5, 7.0, 7.0)),
                line,
            );
        }

        // First/last dates along the bottom edge.
        let base = inset.origin.y + inset.size.height + 8.0;
        if let (Some(a), Some(b)) = (dates.first(), dates.last()) {
            d.text(
                a,
                Point::new(inset.origin.x, base),
                day::prelude::TextStyle {
                    size: 10.0,
                    color: axis_text(dark),
                    anchor: TextAnchor::Leading,
                },
            );
            // No trailing anchor in the canvas text API — center the label just inside
            // the right edge instead.
            d.text(
                b,
                Point::new(inset.origin.x + inset.size.width - 30.0, base + 5.0),
                day::prelude::TextStyle {
                    size: 10.0,
                    color: axis_text(dark),
                    anchor: TextAnchor::Centered,
                },
            );
        }
    })
    .height(280.0)
    .grow_w()
}

/// The volume strip under the chart: one bar per day, trend-colored by that day's direction.
pub fn volume_strip(quote: Signal<day::reactive::Load<crate::quotes::Quote>>) -> AnyPiece {
    let range = crate::quotes::range();
    canvas(move |d, size| {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return;
        };
        let (_, days) = RANGES[range.get().min(RANGES.len() - 1)];
        let closes = crate::quotes::tail(&q.closes, days);
        let volumes = crate::quotes::tail(&q.volumes, days);
        if closes.len() < 2 || size.width < 40.0 {
            return;
        }
        let w = (size.width - 56.0).max(10.0);
        let peak = volumes.iter().cloned().fold(1.0, f64::max);
        let bar_w = (w / volumes.len() as f64).max(1.0);
        for (i, v) in volumes.iter().enumerate() {
            let h = (v / peak) * (size.height - 4.0);
            let up = i == 0 || closes[i] >= closes[i - 1];
            d.fill(
                Shape::Rect(Rect::new(
                    i as f64 * bar_w,
                    size.height - h,
                    (bar_w - 1.0).max(0.75),
                    h,
                )),
                faded(trend_color(up), 0.55),
            );
        }
    })
    .height(56.0)
    .grow_w()
}

/// A watchlist-row sparkline: the last month as a tiny line + soft fill, trend-colored.
pub fn sparkline(quote: Signal<day::reactive::Load<crate::quotes::Quote>>) -> AnyPiece {
    canvas(move |d, size| {
        let Some(q) = quote.with(|l| l.ready().cloned()) else {
            return;
        };
        let closes = crate::quotes::tail(&q.closes, 22);
        if closes.len() < 2 {
            return;
        }
        let (min, max) = closes
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let rect = Rect::new(0.0, 2.0, size.width, size.height - 4.0);
        let pts = project(closes, min, max, rect);
        // Colored by the DAY change, matching the row's chip (the Stocks convention),
        // not by the sparkline window's own trend.
        let line = trend_color(q.change() >= 0.0);
        let mut area = pts.clone();
        area.push(Point::new(size.width, size.height));
        area.push(Point::new(0.0, size.height));
        d.fill(
            Shape::Polygon(area),
            LinearGradient::vertical(faded(line, 0.25), faded(line, 0.0)),
        );
        polyline(d, &pts, line, 1.5);
    })
    .frame(84.0, 30.0)
}

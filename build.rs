//! Generates typed resource constants from this app's `resource/` directory (§18.5).
//!
//! `day-build` scans `resource/{images,assets,fonts}` and writes `$OUT_DIR/day_resources.rs`, which
//! `src/lib.rs` surfaces as the `res` module. App code then references bundled resources by a
//! compiler-checked symbol — `image(res::images::app_logo)` — instead of a bare string: a typo is a
//! build error, the resource is guaranteed bundled, and the available names autocomplete. Adding or
//! removing a file under `resource/` regenerates on the next build.
fn main() {
    day_build::generate_resources().expect("day-build: resource codegen");
    println!("cargo:rustc-env=DAY_TRADR_BUILD_DATE={}", build_date());
}

// Stamped into the settings About section: the build date (UTC, ISO), from
// SOURCE_DATE_EPOCH when a reproducible-build harness sets it, else now. Hand-rolled
// days-to-civil (Howard Hinnant) rather than a chrono dependency for one stamp.
fn build_date() -> String {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

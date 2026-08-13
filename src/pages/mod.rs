//! One module per navigation destination (https://daybrite.dev/docs/navigation).

mod detail;
mod manage;
mod settings;
mod watchlist;

pub use detail::detail_page;
pub use manage::{install_app_menu, manage_page, prompt_for_symbol};
pub use settings::{apply_startup, settings_page};
pub use watchlist::watchlist_page;

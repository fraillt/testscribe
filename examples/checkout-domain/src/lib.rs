mod error;
mod service;
mod status;

use sqlx::migrate::Migrator;

pub use error::DomainError;
pub use service::CheckoutService;

pub static MIGRATOR: Migrator = sqlx::migrate!();

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("entity not found: {0}")]
    NotFound(&'static str),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("insufficient stock for product {product_id}")]
    InsufficientStock { product_id: Uuid },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

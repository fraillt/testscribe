use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DomainError;
use crate::status::{
    CART_ACTIVE, CART_CHECKED_OUT, ORDER_CANCELLED, ORDER_DELIVERED, ORDER_PAID,
    ORDER_PARTIALLY_REFUNDED, ORDER_PLACED, ORDER_REFUNDED, ORDER_SHIPPED, PAYMENT_CAPTURED,
    PAYMENT_FAILED, PAYMENT_PARTIALLY_REFUNDED, PAYMENT_REFUNDED,
};

#[derive(Debug, Clone)]
pub struct CheckoutService {
    pool: PgPool,
}

impl CheckoutService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_customer(&self, email: &str, full_name: &str) -> Result<Uuid, DomainError> {
        let customer_id = Uuid::new_v4();

        sqlx::query!(
            "INSERT INTO customers (id, email, full_name) VALUES ($1, $2, $3)",
            customer_id,
            email,
            full_name
        )
        .execute(&self.pool)
        .await?;

        Ok(customer_id)
    }

    pub async fn create_product(
        &self,
        sku: &str,
        name: &str,
        price_cents: i64,
        stock: i32,
    ) -> Result<Uuid, DomainError> {
        let product_id = Uuid::new_v4();

        sqlx::query!(
            "INSERT INTO products (id, sku, name, price_cents, available_stock) VALUES ($1, $2, $3, $4, $5)",
            product_id,
            sku,
            name,
            price_cents,
            stock
        )
        .execute(&self.pool)
        .await?;

        Ok(product_id)
    }

    pub async fn create_or_load_active_cart(&self, customer_id: Uuid) -> Result<Uuid, DomainError> {
        let existing = sqlx::query!(
            "SELECT id FROM carts WHERE customer_id = $1 AND status = $2 LIMIT 1",
            customer_id,
            CART_ACTIVE
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            return Ok(row.id);
        }

        let cart_id = Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO carts (id, customer_id, status) VALUES ($1, $2, $3)",
            cart_id,
            customer_id,
            CART_ACTIVE
        )
        .execute(&self.pool)
        .await?;

        Ok(cart_id)
    }

    pub async fn add_item_to_cart(
        &self,
        cart_id: Uuid,
        product_id: Uuid,
        quantity: i32,
    ) -> Result<(), DomainError> {
        let cart_status = sqlx::query!(
            "SELECT status as \"status!\" FROM carts WHERE id = $1",
            cart_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(cart_row) = cart_status else {
            return Err(DomainError::NotFound("cart"));
        };
        if cart_row.status != CART_ACTIVE {
            return Err(DomainError::InvalidState(
                "can only add items to active cart",
            ));
        }

        let product = sqlx::query!(
            "SELECT price_cents as \"price_cents!\" FROM products WHERE id = $1",
            product_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(product_row) = product else {
            return Err(DomainError::NotFound("product"));
        };

        let unit_price_cents = product_row.price_cents;
        let item_id = Uuid::new_v4();

        sqlx::query!(
            "INSERT INTO cart_items (id, cart_id, product_id, quantity, unit_price_cents)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (cart_id, product_id)
             DO UPDATE SET quantity = cart_items.quantity + EXCLUDED.quantity,
                           unit_price_cents = EXCLUDED.unit_price_cents",
            item_id,
            cart_id,
            product_id,
            quantity,
            unit_price_cents
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn checkout_cart(&self, cart_id: Uuid) -> Result<Uuid, DomainError> {
        let mut tx = self.pool.begin().await?;

        let cart = sqlx::query!(
            "SELECT customer_id as \"customer_id!\", status as \"status!\" FROM carts WHERE id = $1",
            cart_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(cart_row) = cart else {
            return Err(DomainError::NotFound("cart"));
        };

        let customer_id = cart_row.customer_id;
        if cart_row.status != CART_ACTIVE {
            return Err(DomainError::InvalidState("cart must be active to checkout"));
        }

        let items = sqlx::query!(
            "SELECT
                ci.product_id as \"product_id!\",
                ci.quantity as \"quantity!\",
                ci.unit_price_cents as \"unit_price_cents!\",
                p.available_stock as \"available_stock!\"
             FROM cart_items ci
             JOIN products p ON p.id = ci.product_id
             WHERE ci.cart_id = $1
             FOR UPDATE OF p",
            cart_id
        )
        .fetch_all(&mut *tx)
        .await?;

        if items.is_empty() {
            return Err(DomainError::InvalidState("cart is empty"));
        }

        let mut subtotal_cents = 0_i64;
        for row in &items {
            if row.available_stock < row.quantity {
                return Err(DomainError::InsufficientStock {
                    product_id: row.product_id,
                });
            }

            subtotal_cents += row.unit_price_cents * i64::from(row.quantity);
        }

        for row in &items {
            sqlx::query!(
                "UPDATE products SET available_stock = available_stock - $1 WHERE id = $2",
                row.quantity,
                row.product_id
            )
            .execute(&mut *tx)
            .await?;
        }

        let order_id = Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO orders (id, customer_id, cart_id, status, subtotal_cents)
             VALUES ($1, $2, $3, $4, $5)",
            order_id,
            customer_id,
            cart_id,
            ORDER_PLACED,
            subtotal_cents
        )
        .execute(&mut *tx)
        .await?;

        for row in &items {
            let order_item_id = Uuid::new_v4();

            sqlx::query!(
                "INSERT INTO order_items (id, order_id, product_id, quantity, unit_price_cents)
                 VALUES ($1, $2, $3, $4, $5)",
                order_item_id,
                order_id,
                row.product_id,
                row.quantity,
                row.unit_price_cents
            )
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query!(
            "UPDATE carts SET status = $1, updated_at = NOW() WHERE id = $2",
            CART_CHECKED_OUT,
            cart_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
             VALUES ($1, $2, $3, $4, '{}'::jsonb)",
            Uuid::new_v4(),
            customer_id,
            order_id,
            "order_placed"
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(order_id)
    }

    pub async fn attempt_payment(
        &self,
        order_id: Uuid,
        success: bool,
        provider_reference: Option<&str>,
        failure_reason: Option<&str>,
    ) -> Result<Uuid, DomainError> {
        let mut tx = self.pool.begin().await?;

        let order = sqlx::query!(
            "SELECT
                customer_id as \"customer_id!\",
                status as \"status!\",
                subtotal_cents as \"subtotal_cents!\"
             FROM orders
             WHERE id = $1",
            order_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(order_row) = order else {
            return Err(DomainError::NotFound("order"));
        };

        if order_row.status != ORDER_PLACED {
            return Err(DomainError::InvalidState(
                "payment can only be attempted for placed orders",
            ));
        }

        let payment_id = Uuid::new_v4();
        if success {
            sqlx::query!(
                "INSERT INTO payments (id, order_id, status, provider_reference, amount_cents, failure_reason)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                payment_id,
                order_id,
                PAYMENT_CAPTURED,
                provider_reference,
                order_row.subtotal_cents,
                None::<String>
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query!(
                "UPDATE orders SET status = $1, updated_at = NOW() WHERE id = $2",
                ORDER_PAID,
                order_id
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query!(
                "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
                 VALUES ($1, $2, $3, $4, '{}'::jsonb)",
                Uuid::new_v4(),
                order_row.customer_id,
                order_id,
                "payment_captured"
            )
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query!(
                "INSERT INTO payments (id, order_id, status, provider_reference, amount_cents, failure_reason)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                payment_id,
                order_id,
                PAYMENT_FAILED,
                provider_reference,
                order_row.subtotal_cents,
                failure_reason
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query!(
                "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
                 VALUES ($1, $2, $3, $4, '{}'::jsonb)",
                Uuid::new_v4(),
                order_row.customer_id,
                order_id,
                "payment_failed"
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(payment_id)
    }

    pub async fn ship_order(&self, order_id: Uuid) -> Result<(), DomainError> {
        self.transition_order(order_id, ORDER_PAID, ORDER_SHIPPED, "order_shipped")
            .await
    }

    pub async fn deliver_order(&self, order_id: Uuid) -> Result<(), DomainError> {
        self.transition_order(order_id, ORDER_SHIPPED, ORDER_DELIVERED, "order_delivered")
            .await
    }

    pub async fn cancel_order(&self, order_id: Uuid, _reason: &str) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await?;

        let order = sqlx::query!(
            "SELECT customer_id as \"customer_id!\", status as \"status!\" FROM orders WHERE id = $1",
            order_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(order_row) = order else {
            return Err(DomainError::NotFound("order"));
        };

        if order_row.status != ORDER_PLACED && order_row.status != ORDER_PAID {
            return Err(DomainError::InvalidState(
                "only placed or paid orders can be cancelled",
            ));
        }

        sqlx::query!(
            "UPDATE products p
             SET available_stock = p.available_stock + oi.quantity
             FROM order_items oi
             WHERE oi.order_id = $1 AND p.id = oi.product_id",
            order_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "UPDATE orders SET status = $1, updated_at = NOW() WHERE id = $2",
            ORDER_CANCELLED,
            order_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
             VALUES ($1, $2, $3, $4, '{}'::jsonb)",
            Uuid::new_v4(),
            order_row.customer_id,
            order_id,
            "order_cancelled"
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn refund_order(
        &self,
        order_id: Uuid,
        amount_cents: i64,
        partial: bool,
    ) -> Result<Uuid, DomainError> {
        let mut tx = self.pool.begin().await?;

        let order = sqlx::query!(
            "SELECT
                customer_id as \"customer_id!\",
                status as \"status!\",
                subtotal_cents as \"subtotal_cents!\"
             FROM orders
             WHERE id = $1",
            order_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(order_row) = order else {
            return Err(DomainError::NotFound("order"));
        };

        if order_row.status != ORDER_PAID
            && order_row.status != ORDER_SHIPPED
            && order_row.status != ORDER_DELIVERED
        {
            return Err(DomainError::InvalidState(
                "only paid/shipped/delivered orders can be refunded",
            ));
        }

        if partial {
            if amount_cents <= 0 || amount_cents >= order_row.subtotal_cents {
                return Err(DomainError::InvalidState(
                    "partial refund amount must be between 1 and subtotal-1",
                ));
            }
        } else if amount_cents != order_row.subtotal_cents {
            return Err(DomainError::InvalidState("full refund must match subtotal"));
        }

        let payment_status = if partial {
            PAYMENT_PARTIALLY_REFUNDED
        } else {
            PAYMENT_REFUNDED
        };
        let order_status = if partial {
            ORDER_PARTIALLY_REFUNDED
        } else {
            ORDER_REFUNDED
        };

        let payment_id = Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO payments (id, order_id, status, provider_reference, amount_cents, failure_reason)
             VALUES ($1, $2, $3, NULL, $4, NULL)",
            payment_id,
            order_id,
            payment_status,
            amount_cents
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "UPDATE orders SET status = $1, updated_at = NOW() WHERE id = $2",
            order_status,
            order_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
             VALUES ($1, $2, $3, $4, '{}'::jsonb)",
            Uuid::new_v4(),
            order_row.customer_id,
            order_id,
            "order_refunded"
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(payment_id)
    }

    pub async fn order_status(&self, order_id: Uuid) -> Result<String, DomainError> {
        let row = sqlx::query!(
            "SELECT status as \"status!\" FROM orders WHERE id = $1",
            order_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(DomainError::NotFound("order"));
        };

        Ok(row.status)
    }

    pub async fn notification_types_for_order(
        &self,
        order_id: Uuid,
    ) -> Result<Vec<String>, DomainError> {
        let rows = sqlx::query!(
            "SELECT notification_type as \"notification_type!\"
             FROM notifications
             WHERE order_id = $1
             ORDER BY created_at ASC",
            order_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| row.notification_type).collect())
    }

    async fn transition_order(
        &self,
        order_id: Uuid,
        expected_from: &'static str,
        next: &'static str,
        notification: &'static str,
    ) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await?;

        let order = sqlx::query!(
            "SELECT customer_id as \"customer_id!\", status as \"status!\" FROM orders WHERE id = $1",
            order_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(order_row) = order else {
            return Err(DomainError::NotFound("order"));
        };

        if order_row.status != expected_from {
            return Err(DomainError::InvalidState("invalid order transition"));
        }

        sqlx::query!(
            "UPDATE orders SET status = $1, updated_at = NOW() WHERE id = $2",
            next,
            order_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO notifications (id, customer_id, order_id, notification_type, payload)
             VALUES ($1, $2, $3, $4, '{}'::jsonb)",
            Uuid::new_v4(),
            order_row.customer_id,
            order_id,
            notification
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
}

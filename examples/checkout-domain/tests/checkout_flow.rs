//! Checkout domain test tree.
//!
//! Requires Docker: the environment boots a disposable Postgres via testcontainers.
//! Run with `cargo test -p checkout-domain -- --nocapture`.
//!
//! Tree shape (each node is one business action building on its parent's state):
//!
//! shop_opened_with_customer_and_stocked_products
//! └── items_added_to_cart
//!     ├── same_product_added_again
//!     ├── unavailable_quantity_added_to_cart
//!     └── cart_checked_out
//!         ├── order_cancelled_before_payment
//!         ├── payment_attempt_failed
//!         │   └── payment_retried_with_success
//!         └── payment_captured
//!             ├── order_cancelled_after_payment
//!             ├── order_partially_refunded
//!             └── order_shipped
//!                 └── order_delivered
//!                     └── delivered_order_fully_refunded

use std::sync::Arc;

use checkout_domain::{CheckoutService, DomainError, MIGRATOR};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::{ContainerAsync, runners::AsyncRunner};
use testscribe::prelude::*;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Environment: one Postgres container per tree, one database per branch
// ---------------------------------------------------------------------------

struct ShopEnv {
    service: CheckoutService,
    admin: PgPool,
    db_name: String,
    port: u16,
    container: Arc<ContainerAsync<Postgres>>,
}

impl Environment for ShopEnv {
    type Base = ();

    async fn create(_: Self::Base) -> Self {
        let container = Postgres::default()
            .start()
            .await
            .expect("start postgres container (is Docker running?)");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("get mapped postgres port");
        let admin = connect(port, "postgres").await;
        sqlx::raw_sql("CREATE DATABASE shop")
            .execute(&admin)
            .await
            .expect("create shop database");
        let pool = connect(port, "shop").await;
        MIGRATOR.run(&pool).await.expect("run migrations");
        ShopEnv {
            service: CheckoutService::new(pool),
            admin,
            db_name: "shop".to_owned(),
            port,
            container: Arc::new(container),
        }
    }
}

/// Branching reuses the current database as a template instead of re-running all parent tests,
/// so each sibling continues from an identical, isolated copy.
///
/// A cloneable node's result is a read-only template that no test runs on again, so we can
/// simply close its pool to release every session before snapshotting — Postgres refuses
/// `CREATE DATABASE ... TEMPLATE` while any session is connected to the template.
impl CloneAsync for ShopEnv {
    async fn clone_async(&self) -> Self {
        self.service.pool().close().await;
        let db_name = format!("shop_{}", Uuid::new_v4().simple());
        sqlx::raw_sql(&format!(
            "CREATE DATABASE \"{db_name}\" TEMPLATE \"{}\"",
            self.db_name
        ))
        .execute(&self.admin)
        .await
        .expect("clone database from template");
        let pool = connect(self.port, &db_name).await;
        ShopEnv {
            service: CheckoutService::new(pool),
            admin: self.admin.clone(),
            db_name,
            port: self.port,
            container: Arc::clone(&self.container),
        }
    }
}

async fn connect(port: u16, db: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&format!(
            "postgres://postgres:postgres@127.0.0.1:{port}/{db}"
        ))
        .await
        .expect("connect to postgres")
}

// ---------------------------------------------------------------------------
// Test state: business ids produced by actions, passed from parent to child
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Shop {
    customer: Uuid,
    keyboard: Uuid,
    mouse: Uuid,
}

#[derive(Debug, Clone, CloneAsync)]
struct CartReady {
    shop: Shop,
    cart: Uuid,
}

#[derive(Debug, Clone, CloneAsync)]
struct OrderFlow {
    shop: Shop,
    order: Uuid,
}

// ---------------------------------------------------------------------------
// Custom checks: domain errors as readable assertions
// ---------------------------------------------------------------------------

trait CheckDomainError {
    fn rejected_as_invalid_state(self, reason: &str);
    fn rejected_as_not_found(self, entity: &str);
    fn rejected_for_insufficient_stock(self, product: Uuid);
}

impl<T> CheckDomainError for VerifyValue<'_, Result<T, DomainError>> {
    fn rejected_as_invalid_state(self, reason: &str) {
        let this = VerifyValueExposed::new(self);
        let outcome = match this.actual_value {
            Err(DomainError::InvalidState(actual)) if *actual == reason => VerifyOutcome::Success,
            other => failure(other),
        };
        this.reporter
            .set_outcome(format!("{} is rejected: {reason}", this.var_name), outcome);
    }

    fn rejected_as_not_found(self, entity: &str) {
        let this = VerifyValueExposed::new(self);
        let outcome = match this.actual_value {
            Err(DomainError::NotFound(actual)) if *actual == entity => VerifyOutcome::Success,
            other => failure(other),
        };
        this.reporter.set_outcome(
            format!("{} is rejected: {entity} not found", this.var_name),
            outcome,
        );
    }

    fn rejected_for_insufficient_stock(self, product: Uuid) {
        let this = VerifyValueExposed::new(self);
        let outcome = match this.actual_value {
            Err(DomainError::InsufficientStock { product_id }) if *product_id == product => {
                VerifyOutcome::Success
            }
            other => failure(other),
        };
        this.reporter.set_outcome(
            format!("{} is rejected: insufficient stock", this.var_name),
            outcome,
        );
    }
}

fn failure<T>(actual: &Result<T, DomainError>) -> VerifyOutcome {
    VerifyOutcome::Failure {
        details: match actual {
            Ok(_) => "action unexpectedly succeeded".to_owned(),
            Err(err) => format!("actual error: {err}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Read helpers: direct queries for state the service does not expose
// ---------------------------------------------------------------------------

async fn product_stock(service: &CheckoutService, product: Uuid) -> i32 {
    sqlx::query_scalar("SELECT available_stock FROM products WHERE id = $1")
        .bind(product)
        .fetch_one(service.pool())
        .await
        .expect("query product stock")
}

async fn cart_quantity(service: &CheckoutService, cart: Uuid, product: Uuid) -> i32 {
    sqlx::query_scalar("SELECT quantity FROM cart_items WHERE cart_id = $1 AND product_id = $2")
        .bind(cart)
        .bind(product)
        .fetch_one(service.pool())
        .await
        .expect("query cart item quantity")
}

async fn cart_status(service: &CheckoutService, cart: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM carts WHERE id = $1")
        .bind(cart)
        .fetch_one(service.pool())
        .await
        .expect("query cart status")
}

async fn order_subtotal(service: &CheckoutService, order: Uuid) -> i64 {
    sqlx::query_scalar("SELECT subtotal_cents FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(service.pool())
        .await
        .expect("query order subtotal")
}

async fn payment_statuses(service: &CheckoutService, order: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT status FROM payments WHERE order_id = $1 ORDER BY created_at")
        .bind(order)
        .fetch_all(service.pool())
        .await
        .expect("query payment statuses")
}

// ---------------------------------------------------------------------------
// The test tree
// ---------------------------------------------------------------------------

/// Root: the shop opens with one customer and a small stocked catalog.
#[testscribe(standalone(tokio::test))]
async fn shop_opened_with_customer_and_stocked_products(env: Env<'_, ShopEnv>) -> Shop {
    let svc = &env.service;
    let customer = svc
        .create_customer("alice@example.com", "Alice Cooper")
        .await
        .expect("create customer");
    let keyboard = svc
        .create_product("KB-01", "Mechanical keyboard", 4500, 5)
        .await
        .expect("create keyboard");
    let mouse = svc
        .create_product("MS-01", "Wireless mouse", 1500, 1)
        .await
        .expect("create mouse");

    let keyboard_stock = product_stock(svc, keyboard).await;
    let mouse_stock = product_stock(svc, mouse).await;
    then!(keyboard_stock).eq(5);
    then!(mouse_stock).eq(1);

    Shop {
        customer,
        keyboard,
        mouse,
    }
}

/// Two keyboards and the last mouse in stock land in the customer's active cart.
#[testscribe(cloneable_async)]
async fn items_added_to_cart(
    state: Given<ShopOpenedWithCustomerAndStockedProducts>,
    env: Env<'_, ShopEnv>,
) -> CartReady {
    let svc = &env.service;
    let cart = svc
        .create_or_load_active_cart(state.customer)
        .await
        .expect("create active cart");
    svc.add_item_to_cart(cart, state.keyboard, 2)
        .await
        .expect("add keyboards to cart");
    svc.add_item_to_cart(cart, state.mouse, 1)
        .await
        .expect("add mouse to cart");

    let keyboard_quantity = cart_quantity(svc, cart, state.keyboard).await;
    let mouse_quantity = cart_quantity(svc, cart, state.mouse).await;
    then!(keyboard_quantity).eq(2);
    then!(mouse_quantity).eq(1);

    // stock is only reserved at checkout, not while the cart is filled
    let keyboard_stock = product_stock(svc, state.keyboard).await;
    then!(keyboard_stock).eq(5);

    let unknown_cart = svc
        .add_item_to_cart(Uuid::new_v4(), state.keyboard, 1)
        .await;
    then!(unknown_cart).rejected_as_not_found("cart");

    CartReady {
        shop: state.into_inner(),
        cart,
    }
}

/// Adding the same product again stacks quantity onto the existing cart line.
#[testscribe]
async fn same_product_added_again(state: Given<ItemsAddedToCart>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.add_item_to_cart(state.cart, state.shop.keyboard, 1)
        .await
        .expect("add keyboard again");

    let keyboard_quantity = cart_quantity(svc, state.cart, state.shop.keyboard).await;
    then!(keyboard_quantity).eq(3);

    let cart_lines: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cart_items WHERE cart_id = $1")
        .bind(state.cart)
        .fetch_one(svc.pool())
        .await
        .expect("count cart lines");
    then!(cart_lines).eq(2);
}

/// The customer wants more mice than the shop has; the cart accepts it,
/// but checkout becomes impossible until the quantity fits the stock.
#[testscribe]
async fn unavailable_quantity_added_to_cart(state: Given<ItemsAddedToCart>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.add_item_to_cart(state.cart, state.shop.mouse, 2)
        .await
        .expect("add more mice than in stock");

    let mouse_quantity = cart_quantity(svc, state.cart, state.shop.mouse).await;
    then!(mouse_quantity).eq(3);

    let checkout = svc.checkout_cart(state.cart).await;
    then!(checkout).rejected_for_insufficient_stock(state.shop.mouse);

    // the rejected checkout left no traces
    let mouse_stock = product_stock(svc, state.shop.mouse).await;
    then!(mouse_stock).eq(1);
    let status = cart_status(svc, state.cart).await;
    then!(status => cart_status).eq("active");
}

/// Checkout reserves stock, places the order and freezes the cart.
#[testscribe(cloneable_async)]
async fn cart_checked_out(state: Given<ItemsAddedToCart>, env: Env<'_, ShopEnv>) -> OrderFlow {
    let svc = &env.service;
    let order = svc.checkout_cart(state.cart).await.expect("checkout cart");

    let order_status = svc.order_status(order).await.expect("query order status");
    then!(order_status).eq("placed");

    let keyboard_stock = product_stock(svc, state.shop.keyboard).await;
    let mouse_stock = product_stock(svc, state.shop.mouse).await;
    then!(keyboard_stock).eq(3);
    then!(mouse_stock).eq(0);

    let subtotal = order_subtotal(svc, order).await;
    then!(subtotal).eq(2 * 4500 + 1500);

    let status = cart_status(svc, state.cart).await;
    then!(status => cart_status).eq("checked_out");

    let notifications = svc
        .notification_types_for_order(order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec!["order_placed"]);

    // the checked-out cart no longer accepts changes
    let checkout_again = svc.checkout_cart(state.cart).await;
    then!(checkout_again).rejected_as_invalid_state("cart must be active to checkout");
    let add_after_checkout = svc
        .add_item_to_cart(state.cart, state.shop.keyboard, 1)
        .await;
    then!(add_after_checkout).rejected_as_invalid_state("can only add items to active cart");

    OrderFlow {
        shop: state.into_inner().shop,
        order,
    }
}

/// Cancelling before payment returns the reserved stock to the shelf.
#[testscribe]
async fn order_cancelled_before_payment(state: Given<CartCheckedOut>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.cancel_order(state.order, "changed my mind")
        .await
        .expect("cancel order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("cancelled");

    let keyboard_stock = product_stock(svc, state.shop.keyboard).await;
    let mouse_stock = product_stock(svc, state.shop.mouse).await;
    then!(keyboard_stock).eq(5);
    then!(mouse_stock).eq(1);

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec!["order_placed", "order_cancelled"]);

    // a cancelled order is a dead end
    let pay_cancelled = svc
        .attempt_payment(state.order, true, Some("ch_1"), None)
        .await;
    then!(pay_cancelled)
        .rejected_as_invalid_state("payment can only be attempted for placed orders");
    let ship_cancelled = svc.ship_order(state.order).await;
    then!(ship_cancelled).rejected_as_invalid_state("invalid order transition");
}

/// The first payment attempt fails; the order stays placed, waiting for a retry.
#[testscribe]
async fn payment_attempt_failed(state: Given<CartCheckedOut>, env: Env<'_, ShopEnv>) -> OrderFlow {
    let svc = &env.service;
    svc.attempt_payment(state.order, false, None, Some("card declined"))
        .await
        .expect("record failed payment attempt");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("placed");

    let payments = payment_statuses(svc, state.order).await;
    then!(payments).eq(vec!["failed"]);

    then!("failure reason is kept for support")
        .run_async(async || {
            let reason: Option<String> = sqlx::query_scalar(
                "SELECT failure_reason FROM payments WHERE order_id = $1
                 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(state.order)
            .fetch_one(svc.pool())
            .await
            .expect("query failure reason");
            reason.as_deref() == Some("card declined")
        })
        .await;

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec!["order_placed", "payment_failed"]);

    state.into_inner()
}

/// After a decline, the retried payment is captured and the order recovers to paid. This branch
/// exists to prove the failure-recovery path; the full fulfillment journey continues from the
/// first-try `payment_captured` node instead, so the happy path reads start-to-finish.
#[testscribe]
async fn payment_retried_with_success(state: Given<PaymentAttemptFailed>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.attempt_payment(state.order, true, Some("ch_42"), None)
        .await
        .expect("capture payment");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("paid");

    let payments = payment_statuses(svc, state.order).await;
    then!(payments).eq(vec!["failed", "captured"]);

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec!["order_placed", "payment_failed", "payment_captured"]);

    // a paid order cannot be paid twice
    let pay_again = svc
        .attempt_payment(state.order, true, Some("ch_43"), None)
        .await;
    then!(pay_again).rejected_as_invalid_state("payment can only be attempted for placed orders");
}

/// The happy path: the first payment attempt is captured straight away and the order is paid.
/// The full fulfillment journey (ship, deliver, refund, cancel) branches from here.
#[testscribe(cloneable_async)]
async fn payment_captured(state: Given<CartCheckedOut>, env: Env<'_, ShopEnv>) -> OrderFlow {
    let svc = &env.service;
    svc.attempt_payment(state.order, true, Some("ch_1"), None)
        .await
        .expect("capture payment");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("paid");

    let payments = payment_statuses(svc, state.order).await;
    then!(payments).eq(vec!["captured"]);

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec!["order_placed", "payment_captured"]);

    // a paid order cannot be paid twice
    let pay_again = svc
        .attempt_payment(state.order, true, Some("ch_2"), None)
        .await;
    then!(pay_again).rejected_as_invalid_state("payment can only be attempted for placed orders");

    state.into_inner()
}

/// Cancelling a paid order still restores stock.
#[testscribe]
async fn order_cancelled_after_payment(state: Given<PaymentCaptured>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.cancel_order(state.order, "customer request")
        .await
        .expect("cancel paid order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("cancelled");

    let keyboard_stock = product_stock(svc, state.shop.keyboard).await;
    let mouse_stock = product_stock(svc, state.shop.mouse).await;
    then!(keyboard_stock).eq(5);
    then!(mouse_stock).eq(1);
}

/// Part of the paid amount is refunded; the order is marked accordingly.
#[testscribe]
async fn order_partially_refunded(state: Given<PaymentCaptured>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;

    // refund amounts are validated against the subtotal (10500)
    let zero_refund = svc.refund_order(state.order, 0, true).await;
    then!(zero_refund)
        .rejected_as_invalid_state("partial refund amount must be between 1 and subtotal-1");
    let full_as_partial = svc.refund_order(state.order, 10500, true).await;
    then!(full_as_partial)
        .rejected_as_invalid_state("partial refund amount must be between 1 and subtotal-1");
    let wrong_full = svc.refund_order(state.order, 9999, false).await;
    then!(wrong_full).rejected_as_invalid_state("full refund must match subtotal");

    svc.refund_order(state.order, 2000, true)
        .await
        .expect("partially refund order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("partially_refunded");

    let payments = payment_statuses(svc, state.order).await;
    then!(payments).eq(vec!["captured", "partially_refunded"]);

    // a partially refunded order cannot be refunded again
    let refund_again = svc.refund_order(state.order, 2000, true).await;
    then!(refund_again)
        .rejected_as_invalid_state("only paid/shipped/delivered orders can be refunded");
}

/// The paid order is handed to the carrier.
#[testscribe]
async fn order_shipped(state: Given<PaymentCaptured>, env: Env<'_, ShopEnv>) -> OrderFlow {
    let svc = &env.service;
    svc.ship_order(state.order).await.expect("ship order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("shipped");

    // it is too late to cancel once shipped
    let cancel_shipped = svc.cancel_order(state.order, "too late").await;
    then!(cancel_shipped).rejected_as_invalid_state("only placed or paid orders can be cancelled");

    state.into_inner()
}

/// The carrier delivers the order; the whole journey is visible to the customer.
#[testscribe]
async fn order_delivered(state: Given<OrderShipped>, env: Env<'_, ShopEnv>) -> OrderFlow {
    let svc = &env.service;
    svc.deliver_order(state.order).await.expect("deliver order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("delivered");

    #[derive(Clone, ParamDisplay)]
    struct SentNotification {
        kind: &'static str,
    }

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!("customer was notified on every step of the journey")
        .params([
            SentNotification {
                kind: "order_placed",
            },
            SentNotification {
                kind: "payment_captured",
            },
            SentNotification {
                kind: "order_shipped",
            },
            SentNotification {
                kind: "order_delivered",
            },
        ])
        .run(|expected| notifications.iter().any(|kind| kind == expected.kind));

    // delivering twice is not a thing
    let deliver_again = svc.deliver_order(state.order).await;
    then!(deliver_again).rejected_as_invalid_state("invalid order transition");

    state.into_inner()
}

/// Even a delivered order can be fully refunded.
#[testscribe]
async fn delivered_order_fully_refunded(state: Given<OrderDelivered>, env: Env<'_, ShopEnv>) {
    let svc = &env.service;
    svc.refund_order(state.order, 10500, false)
        .await
        .expect("fully refund delivered order");

    let order_status = svc
        .order_status(state.order)
        .await
        .expect("query order status");
    then!(order_status).eq("refunded");

    let payments = payment_statuses(svc, state.order).await;
    then!(payments).eq(vec!["captured", "refunded"]);

    let notifications = svc
        .notification_types_for_order(state.order)
        .await
        .expect("query notifications");
    then!(notifications).eq(vec![
        "order_placed",
        "payment_captured",
        "order_shipped",
        "order_delivered",
        "order_refunded",
    ]);
}

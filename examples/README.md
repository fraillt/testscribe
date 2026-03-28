# Checkout Domain Example (Library-Only)

This example models a small e-commerce checkout system as a **Rust library**.

It is intentionally focused on **business logic**, not on HTTP routes, handlers, or transport concerns. The goal is to make it easy to test domain behavior with `testscribe` while keeping the system realistic enough to expose meaningful state transitions.

## Why this example exists

From a user perspective, checkout looks simple: add items, pay, get the order. From a system perspective, it is a multi-step workflow with many branching outcomes.

This makes checkout a good domain for demonstrating:

- stateful flows
- side effects
- branching behavior
- retries and failure paths
- readable, story-like tests

## Scope

- This project is a **library**, not a binary.
- We do not integrate with HTTP in this example because that adds little value for the testing goals.
- We test domain logic directly through library APIs.

## Actors and relationships

At a high level, the system involves these actors:

- **Customer**: owns carts and places orders.
- **Cart**: contains items selected by a customer.
- **Inventory**: tracks stock availability for products.
- **Order**: represents checkout progress and order lifecycle state.
- **Payment processor** (integration boundary): authorizes/captures/fails payment attempts.
- **Notification system** (integration boundary): sends user-facing events (for example, payment failed, order shipped).

Relationships:

- A customer has one active cart and can create many orders over time.
- An order is created from a cart snapshot.
- Inventory is reserved/consumed as order state changes.
- Payment outcomes drive order transitions.
- Notifications are triggered by important state changes.

## What the system should do (high level)

The library should support a realistic order lifecycle:

1. Create or load a customer cart.
2. Validate inventory and create an order.
3. Attempt payment (success/failure/retry).
4. Move order through fulfillment states.
5. Handle cancellation/refund scenarios.
6. Emit side effects (notifications, audit/event records).

The exact implementation details are intentionally simple, but state transitions and invariants should remain strict.

## Persistence and realism

Even though this is a library-focused example, all domain data should be persisted in a database.

Why:

- It keeps tests closer to production behavior.
- It surfaces realistic state and consistency issues.
- It makes tree-based, state-reuse tests more representative.

The intention is to test logic against persisted state, not mock everything in memory.

## Testing direction

This domain will be used to demonstrate `testscribe` patterns:

- one logical action per test
- parent/child state reuse through test trees
- clear verification output for state transitions and side effects

## Development

This example is developed as a library crate in `examples/checkout-domain`.

### Prerequisites

- Rust toolchain installed
- `sqlx-cli` installed (`cargo install sqlx-cli --no-default-features --features rustls,postgres`)
- Local PostgreSQL instance available

### Local environment

The crate includes a local `.env` file:
- `cp .env .env.sample`
And modify connection to PostgreSQL

### Create database, run migrations and refresh local sqlx cache

```console
cargo sqlx database reset
cargo sqlx prepare
```

## Why standard Rust's libtest is not enough

For simple unit tests with little or no complex state, Rust's standard `libtest` works great. For example, `fn test_email_parsing()` is easy to understand, and we can assume it covers valid and invalid email formats. We still do not know the exact cases from the name alone, but we at least get a clear high-level idea.

However, once a test becomes non-trivial and requires a specific initial state, the situation changes. A test like `fn test_failed_payment()` raises more questions than it answers. Without reading the implementation, it is hard to know what is actually being verified.

Sometimes naming helps, for example `fn when_payment_is_failed_then_send_email_to_user()`. But even then it's unclear if there are other steps between failure and sending email (maybe payment should be retried first), are these cases covered with tests?

This gets worse when setup is expensive (for example, starting multiple containers with [testcontainers](https://testcontainers.com/)). In that case, teams often trade clarity for execution time and pack too much logic into a single test. In complex systems, it is common to see tests like `fn test_client_configuration()` that include many client-related behaviors and take a minute or more to run.

This is where `testscribe` comes in.

## What I want instead

I want to open test output and quickly understand the scenario: how state was created, what action happened, and what was checked/verified.

I want to avoid repeating expensive setup for every assertion while still keeping each test focused on one meaningful action.

I want an easy start with no extra steps (e.g., feature files in some BDD frameworks).

In practice, that means:

- tests should be composed from previous steps instead of rebuilding full state each time;
- each test should describe one business-relevant transition;
- checks/assertions should be explicit and readable in source code and in test output;
- adding a new scenario should feel like extending a story branch, not duplicating boilerplate;
- to get started, just add the `#[testscribe(standalone)]` attribute to an existing test.

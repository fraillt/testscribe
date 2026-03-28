## Test writing checklist

Here is a concise checklist to help you write tests efficiently. The rationale for these points is covered in other documentation.

After writing a test function, go through this list and check whether all answers are "yes":

- [ ] Does it test only one business/domain action?
- [ ] Does the test function perform side effects?
- [ ] Does the test function name sound like an event?
- [ ] Does it reuse existing state from a parent test (except for a root test)?
- [ ] Does it check all important side effects that happened?
- [ ] Does it check all important constraints that should not happen?
- [ ] Does it enable new actions (to create new tests from)?
- [ ] Are checks/assertions efficient and convenient to write?
- [ ] Is test output easy to understand (human-readable)?

Additional questions to consider after writing more tests:

- [ ] Should test state and environment state be separated?
- [ ] Should custom assertions/checks be introduced?
- [ ] Should execution speed be improved by cloning test state (and environment state)?

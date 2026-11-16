# Milestone 1

DONE!

In short, implement frontend which would run all the tests.

# Milestone 2

DONE!

"Test batch" feature
* All the tests ran with `TESTSCRIBE_FRONTEND=127.0.0.1:7357 cargo test -p testscribe --features detached` will be stored in database (lets start with sqlite). The batch it self can be specified with `TESTSCRIBE_BATCH_ID` env variable, which would be any string.
  - without a batch id the batch is `default`, which is kept in memory only and never stored
* Provide a way to diff different batches, by writing added/deleted/modified tests between the batches
  - tests are compared by the text of their outcome, as it is printed. Where a test or a check is in the sources is not part of it, so moving code around changes nothing
* This feature can also be used while developing, by describing behaviour how to behave when reruning the same test batch:
  - run all - default nothing special simple resets current batch and runs everything
  - run selected test(s) - in the UI you can select which tests you want to run, and only these tests will be ran
  - run new-and-failed - useful in development, only runs new or failed tests for same batch
  - a rerun only overwrites the tests that actually ran

# Milestone 3

DONE!

Things that came up while using milestone 2.
* executions and times of a test and its branches, with the setup time derived from the test timings; hidden unless "show times" is ticked
* run the tests of a batch from the UI
* a batch knows the tests that have not run, so that a test that did not run is not told as deleted
* `TESTSCRIBE_RUN` says what a run executes, from the terminal
* a batch as a file (`export`, `import`), and `diff` against it as a gate in CI
* compare the times of two batches
* the git commit and branch a batch last ran on
* filters and search on the page of a batch, links to the sources, slowest first
* the page of a batch follows the tests while they run
* the frontend is built and tested in CI

See [design.md](design.md) for the decisions, and for what is known not to work.

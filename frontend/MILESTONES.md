# Milestone 1

DONE!

In short, implement frontend which would run all the tests.

# Milestone 2

"Test batch" feature
* All the tests ran with `TESTSCRIBE_FRONTEND=127.0.0.1:7357 cargo test -p testscribe --features detached` will be stored in database (lets start with sqlite). The batch it self can be specified with `TESTSCRIBE_BATCH_ID` env variable, which would be any string.
* Provide a way to diff different batches, by writing added/deleted/modified tests between the batches
* This feature can also be used while developing, by describing behaviour how to behave when reruning the same test batch:
  - run all - default nothing special simple resets current batch and runs everything
  - run selected test(s) - in the UI you can select which tests you want to run, and only these tests will be ran
  - run new-and-failed - useful in development, only runs new or failed tests for same batch
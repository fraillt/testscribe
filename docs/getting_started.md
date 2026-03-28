## Getting started

This will walk you through whole library functionality starting from basic library features, and finishing with advanced topics to write more efficient tests.

### Core features


### Advanced features


### Tips to be successful with `testscribe`

1. The name of the function should reflect the action(s) it performs. Keep in mind that most test builds on previous test state, so name should be conside, as it's usually easy to understand by looking at their parents what exactly is going on.
2. `testscribe` comes with basic verify functions. Create your own ones for your specific domain for more readable code and test outcome.
3. Don't look into code to discover untested parts. Instead simply run the tests and read the outcome (tests tree) and identify which actions at which states are not covered yet. 

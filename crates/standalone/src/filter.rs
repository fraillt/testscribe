use testscribe_core::test_case::TestCase;

use crate::args::Arguments;

pub const IGNORE_TAG_NAME: &str = "ignore";

pub fn filter_out_test(test: &TestCase, args: &Arguments) -> bool {
    // search works both for filter and skip in few modes:
    // if search string is wrapped in [], then it's tag search, and it must be exact match
    // otherwise, it's name search, and it can be exact (if exact is specified) or partial match
    if args.filter.is_some() || !args.skip.is_empty() {
        let test_name = format!("{}", test.name);

        if let Some(filter) = &args.filter {
            if !match_test(&test_name, test.tags, filter, args.exact) {
                return true;
            }
        }

        for filter in &args.skip {
            if match_test(&test_name, test.tags, filter, args.exact) {
                return true;
            }
        }
    }
    args.ignored && !test.tags.contains(&IGNORE_TAG_NAME)
}

fn match_test(test_name: &str, tags: &[&'static str], filter: &str, exact: bool) -> bool {
    let by_tag = filter
        .strip_prefix("[")
        .and_then(|filter| filter.strip_suffix("]"));
    if let Some(tag_filter) = by_tag {
        tags.iter().any(|tag| *tag == tag_filter)
    } else if exact {
        test_name == filter
    } else {
        test_name.contains(filter)
    }
}

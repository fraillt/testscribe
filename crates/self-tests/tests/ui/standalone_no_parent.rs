use testscribe::testscribe;

#[testscribe]
fn parent_test() {
    then!("");
}

#[testscribe(standalone)]
fn standalone(_: Given<ParentTest>) {
    then!("");
}

fn main() {}

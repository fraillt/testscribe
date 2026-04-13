use testscribe::testscribe;

#[testscribe(standalone)]
async fn async_root_without_harness() {
    then!("");
}

fn main() {}

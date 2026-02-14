use testscribe::testscribe;

#[testscribe(cloneable, cloneable_async)]
fn my_test() {
    then!("");
}

fn main() {}
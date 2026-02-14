use testscribe::testscribe;

#[testscribe(tags=[fasd,fea], unknown)]
fn my_test() {
    then!("");
}

fn main() {}
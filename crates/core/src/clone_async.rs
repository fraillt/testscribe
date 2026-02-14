pub trait CloneAsync {
    fn clone_async(&self) -> impl Future<Output = Self>;
}

impl CloneAsync for () {
    fn clone_async(&self) -> impl Future<Output = Self> {
        async {}
    }
}

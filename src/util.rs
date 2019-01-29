pub trait WithSideEffects {
    type WrappedType;

    fn with_side_effects<F>(self, f: F) -> Self
    where
        F: FnOnce(Self::WrappedType);
}

impl<T: Copy> WithSideEffects for Option<T> {
    type WrappedType = T;

    fn with_side_effects<F>(self, f: F) -> Self
    where
        F: FnOnce(Self::WrappedType),
    {
        self.map(|value| {
            f(value);
            value
        })
    }
}

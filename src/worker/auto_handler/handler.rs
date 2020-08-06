use serde::Serialize;
use std::future::Future;

/// Job handler converter factory
pub trait HandlerFactory<T, R, O, E>: Clone + 'static
where
    R: Future<Output = std::result::Result<O, E>>,
    O: Serialize,
    E: std::error::Error,
{
    fn call(&self, param: T) -> R;
}

/// FromJob trait impl for tuples
macro_rules! factory_tuple ({ $(($n:tt, $T:ident)),+} => {
    impl<Func, $($T,)+ Res, O, ER> HandlerFactory<($($T,)+), Res, O, ER> for Func
    where Func: Fn($($T,)+) -> Res + Clone + 'static,
          Res: Future<Output = std::result::Result<O, ER>>,
          O: Serialize,
          ER: std::error::Error,
    {
        fn call(&self, param: ($($T,)+)) -> Res {
            (self)($(param.$n,)+)
        }
    }
});

#[rustfmt::skip]
mod m {
    use super::*;

    factory_tuple!((0, A));
    factory_tuple!((0, A), (1, B));
    factory_tuple!((0, A), (1, B), (2, C));
    factory_tuple!((0, A), (1, B), (2, C), (3, D));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I));
    factory_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J));
}

use crate::{
    client::Client,
    error::{Error, Result},
    job::Job,
};

mod data;
mod state;

pub use data::Data;
pub use state::State;

pub trait FromJob: Sized {
    fn from_job(client: &Client, job: &Job) -> std::result::Result<Self, Error>;
}

impl FromJob for Client {
    fn from_job(client: &Client, _: &Job) -> std::result::Result<Self, Error> {
        Ok(client.clone())
    }
}

impl FromJob for Job {
    fn from_job(_: &Client, job: &Job) -> std::result::Result<Self, Error> {
        Ok(job.clone())
    }
}

impl<T: serde::de::DeserializeOwned> FromJob for Data<T> {
    fn from_job(_: &Client, job: &Job) -> Result<Self> {
        Ok(Data(serde_json::from_str(job.variables_str())?))
    }
}

impl<T: 'static> FromJob for State<T> {
    fn from_job(client: &Client, _: &Job) -> Result<Self> {
        if let Some(data) = client
            .current_job_extensions
            .as_ref()
            .and_then(|ext| ext.get::<State<T>>())
        {
            Ok(data.clone())
        } else {
            Err(Error::MissingWorkerStateConfig)
        }
    }
}

macro_rules! tuple_from_job ({$(($n:tt, $T:ident)),+} => {
    /// FromJob implementation for tuple
    #[doc(hidden)]
    impl<$($T: FromJob + 'static),+> FromJob for ($($T,)+)
    {
        fn from_job(client: &Client, job: &Job) -> Result<Self> {
            Ok(($($T::from_job(client, job)?,)+))
        }
    }
});

#[rustfmt::skip]
mod m {
    use super::*;

    tuple_from_job!((0, A));
    tuple_from_job!((0, A), (1, B));
    tuple_from_job!((0, A), (1, B), (2, C));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I));
    tuple_from_job!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J));
}

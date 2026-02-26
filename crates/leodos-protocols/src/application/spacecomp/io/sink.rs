use core::future::Future;

use crate::application::spacecomp::schema::Schema;

pub trait Sink {
    type Input: Schema;
    type Error;

    fn write(&mut self, val: &Self::Input) -> impl Future<Output = Result<(), Self::Error>>;
}

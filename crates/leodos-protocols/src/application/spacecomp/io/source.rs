use core::future::Future;

use crate::application::spacecomp::schema::Schema;

pub trait Source {
    type Output: Schema;
    type Error: core::error::Error;

    fn read(&mut self) -> impl Future<Output = Option<Result<Self::Output, Self::Error>>>;
}

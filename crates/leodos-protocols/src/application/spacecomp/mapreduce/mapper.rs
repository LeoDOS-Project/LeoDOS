use core::future::Future;

use bon::Builder;

use crate::application::spacecomp::io::sink::Sink;
use crate::application::spacecomp::io::source::Source;
use crate::application::spacecomp::schema::Schema;

/// A trait for mapping input data to output data.
pub trait Mapper {
    type Input: Schema;
    type Output: Schema;

    fn map<S>(
        &mut self,
        input: Self::Input,
        sink: &mut S,
    ) -> impl Future<Output = Result<(), S::Error>>
    where
        S: Sink<Input = Self::Output>;
}

/// A runner that connects a source, mapper, and sink.
#[derive(Builder)]
pub struct MapRunner<Src, Map, Snk> {
    pub source: Src,
    pub mapper: Map,
    pub sink: Snk,
}

impl<Src, Map, Snk> MapRunner<Src, Map, Snk>
where
    Src: Source,
    Map: Mapper<Input = Src::Output>,
    Snk: Sink<Input = Map::Output>,
{
    pub async fn run(&mut self) -> Result<(), Snk::Error> {
        loop {
            let input = match self.source.read().await {
                None => break,
                Some(Err(_)) => {
                    continue;
                }
                Some(Ok(val)) => val,
            };
            self.mapper.map(input, &mut self.sink).await?;
        }
        Ok(())
    }
}

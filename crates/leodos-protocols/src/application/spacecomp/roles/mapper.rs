use core::future::Future;

use bon::Builder;

use crate::application::spacecomp::io::sink::Sink;
use crate::application::spacecomp::io::source::Source;
use crate::application::spacecomp::schema::Schema;

/// A trait for mapping input data to output data.
pub trait Mapper {
    /// The input schema consumed by this mapper.
    type Input: Schema;
    /// The output schema produced by this mapper.
    type Output: Schema;

    /// Maps a single input value, writing outputs to the sink.
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
    /// Data source providing input values.
    pub source: Src,
    /// Mapper that processes each input value.
    pub mapper: Map,
    /// Sink that receives the mapped output values.
    pub sink: Snk,
}

impl<Src, Map, Snk> MapRunner<Src, Map, Snk>
where
    Src: Source,
    Map: Mapper<Input = Src::Output>,
    Snk: Sink<Input = Map::Output>,
{
    /// Runs the map pipeline to completion.
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

use crate::application::spacecomp::io::sink::Sink;
use crate::application::spacecomp::io::source::Source;
use crate::application::spacecomp::schema::Schema;

/// A trait for reducing input data to output data.
pub trait Reducer {
    /// The input schema consumed by this reducer.
    type Input: Schema;
    /// The output schema produced by this reducer.
    type Output: Schema;

    /// Reduces a single input value, yielding zero or more outputs.
    fn reduce(&mut self, val: Self::Input) -> impl Iterator<Item = Self::Output>;
}

/// A runner that connects a source, reducer, and sink.
pub struct ReducerRunner<Src, Map, Snk> {
    /// Data source providing input values.
    pub source: Src,
    /// Reducer that processes each input value.
    pub mapper: Map,
    /// Sink that receives the reduced output values.
    pub sink: Snk,
}

impl<Src, Map, Snk> ReducerRunner<Src, Map, Snk>
where
    Map: Reducer,
    Src: Source<Output = Map::Input>,
    Snk: Sink<Input = Map::Output>,
{
    /// Runs the reduce pipeline to completion.
    pub async fn run(mut self) -> Result<(), Snk::Error> {
        while let Some(res) = self.source.read().await {
            match res {
                Err(_) => {
                    // Handle source error
                    continue;
                }
                Ok(v) => {
                    for v in self.mapper.reduce(v) {
                        self.sink.write(&v).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

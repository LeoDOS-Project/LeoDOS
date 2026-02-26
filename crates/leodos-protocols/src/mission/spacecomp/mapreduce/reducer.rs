use crate::mission::spacecomp::io::sink::Sink;
use crate::mission::spacecomp::io::source::Source;
use crate::mission::spacecomp::schema::Schema;

/// A trait for reducing input data to output data.
pub trait Reducer {
    type Input: Schema;
    type Output: Schema;

    fn reduce(&mut self, val: Self::Input) -> impl Iterator<Item = Self::Output>;
}

/// A runner that connects a source, reducer, and sink.
pub struct ReducerRunner<Src, Map, Snk> {
    pub source: Src,
    pub mapper: Map,
    pub sink: Snk,
}

impl<Src, Map, Snk> ReducerRunner<Src, Map, Snk>
where
    Map: Reducer,
    Src: Source<Output = Map::Input>,
    Snk: Sink<Input = Map::Output>,
{
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

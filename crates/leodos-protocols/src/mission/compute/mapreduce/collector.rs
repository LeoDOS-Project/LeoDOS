use core::future::Future;

use bon::Builder;

use crate::mission::compute::io::sink::Sink;
use crate::mission::compute::io::source::Source;
use crate::mission::compute::schema::Schema;

/// A trait for collecting sensor data and producing processed output.
pub trait Collector {
    type Input: Schema;
    type Output: Schema;

    fn collect<S>(
        &mut self,
        input: Self::Input,
        sink: &mut S,
    ) -> impl Future<Output = Result<(), S::Error>>
    where
        S: Sink<Input = Self::Output>;
}

/// A runner that connects a source, collector, and sink.
#[derive(Builder)]
pub struct CollectRunner<Src, Col, Snk> {
    pub source: Src,
    pub collector: Col,
    pub sink: Snk,
}

impl<Src, Col, Snk> CollectRunner<Src, Col, Snk>
where
    Src: Source,
    Col: Collector<Input = Src::Output>,
    Snk: Sink<Input = Col::Output>,
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
            self.collector.collect(input, &mut self.sink).await?;
        }
        Ok(())
    }
}

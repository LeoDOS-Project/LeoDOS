use core::future::Future;

use bon::Builder;

use crate::application::spacecomp::io::sink::Sink;
use crate::application::spacecomp::io::source::Source;
use crate::application::spacecomp::schema::Schema;

/// A trait for collecting sensor data and producing processed output.
pub trait Collector {
    /// The input schema consumed from the sensor source.
    type Input: Schema;
    /// The output schema produced after collection processing.
    type Output: Schema;

    /// Collects a single input value, writing outputs to the sink.
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
    /// Data source providing sensor input values.
    pub source: Src,
    /// Collector that processes each sensor reading.
    pub collector: Col,
    /// Sink that receives the collected output values.
    pub sink: Snk,
}

impl<Src, Col, Snk> CollectRunner<Src, Col, Snk>
where
    Src: Source,
    Col: Collector<Input = Src::Output>,
    Snk: Sink<Input = Col::Output>,
{
    /// Runs the collect pipeline to completion.
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

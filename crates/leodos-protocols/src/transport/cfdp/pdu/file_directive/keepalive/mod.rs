pub mod large;
pub mod small;

#[derive(Debug)]
pub enum KeepAlivePdu<'a> {
    Small(&'a small::KeepAlivePduSmall),
    Large(&'a large::KeepAlivePduLarge),
}

impl<'a> KeepAlivePdu<'a> {
    pub fn progress(&self) -> u64 {
        match self {
            KeepAlivePdu::Small(small) => small.progress() as u64,
            KeepAlivePdu::Large(large) => large.progress(),
        }
    }
}

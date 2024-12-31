use svd_rs::Name;

use super::matchname;

pub struct MatchIter<'b, I>
where
    I: Iterator,
    I::Item: Name,
{
    it: I,
    spec: &'b str,
}

impl<I> Iterator for MatchIter<'_, I>
where
    I: Iterator,
    I::Item: Name,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.it
            .by_ref()
            .find(|next| matchname(next.name(), self.spec))
    }
}

pub trait Matched
where
    Self: Iterator + Sized,
    Self::Item: Name,
{
    fn matched(self, spec: &str) -> MatchIter<Self>;
}

impl<I> Matched for I
where
    Self: Iterator + Sized,
    Self::Item: Name,
{
    fn matched(self, spec: &str) -> MatchIter<Self> {
        MatchIter { it: self, spec }
    }
}

/// Iterates over optional iterator
pub struct OptIter<I>(Option<I>)
where
    I: Iterator;

impl<I> OptIter<I>
where
    I: Iterator,
{
    /// Create new optional iterator
    pub fn new(o: Option<I>) -> Self {
        Self(o)
    }
}

impl<I> Iterator for OptIter<I>
where
    I: Iterator,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(I::next)
    }
}

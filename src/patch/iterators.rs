use svd_parser::svd;

use super::matchname;

pub struct MatchIter<'b, I>
where
    I: Iterator,
    I::Item: GetName,
{
    it: I,
    spec: &'b str,
}

impl<'b, I> Iterator for MatchIter<'b, I>
where
    I: Iterator,
    I::Item: GetName,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        for next in self.it.by_ref() {
            if matchname(next.get_name(), self.spec) {
                return Some(next);
            }
        }
        None
    }
}

pub trait Matched
where
    Self: Iterator + Sized,
    Self::Item: GetName,
{
    fn matched(self, spec: &str) -> MatchIter<Self>;
}

impl<I> Matched for I
where
    Self: Iterator + Sized,
    Self::Item: GetName,
{
    fn matched(self, spec: &str) -> MatchIter<Self> {
        MatchIter { it: self, spec }
    }
}

pub trait GetName {
    fn get_name(&self) -> &str;
}
impl GetName for svd::Interrupt {
    fn get_name(&self) -> &str {
        &self.name
    }
}
impl GetName for svd::Field {
    fn get_name(&self) -> &str {
        &self.name
    }
}
impl GetName for svd::Register {
    fn get_name(&self) -> &str {
        &self.name
    }
}
impl GetName for svd::Cluster {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl<T> GetName for &T
where
    T: GetName,
{
    fn get_name(&self) -> &str {
        T::get_name(*self)
    }
}

impl<T> GetName for &mut T
where
    T: GetName,
{
    fn get_name(&self) -> &str {
        T::get_name(*self)
    }
}

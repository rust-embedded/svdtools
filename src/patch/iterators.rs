use svd_parser::svd;

use super::matchname;

pub struct MatchIterMut<'a, 'b, T: 'a, I>
where
    T: 'a + GetName,
    I: Iterator<Item = &'a mut T>,
{
    it: I,
    spec: &'b str,
}

impl<'a, 'b, T, I> Iterator for MatchIterMut<'a, 'b, T, I>
where
    T: 'a + GetName,
    I: Iterator<Item = &'a mut T>,
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.it.next() {
            if matchname(&next.get_name(), self.spec) {
                return Some(next);
            }
        }
        None
    }
}

pub trait Matched<'a, T: 'a>
where
    Self: Iterator<Item = &'a mut T> + Sized,
    T: 'a,
{
    fn matched<'b>(self, spec: &'b str) -> MatchIterMut<'a, 'b, T, Self>
    where
        T: GetName;
}

impl<'a, T, I> Matched<'a, T> for I
where
    Self: Iterator<Item = &'a mut T> + Sized,
    T: 'a,
{
    fn matched<'b>(self, spec: &'b str) -> MatchIterMut<'a, 'b, T, Self>
    where
        T: GetName,
    {
        MatchIterMut { it: self, spec }
    }
}

pub struct OptIter<T, I>(Option<I>)
where
    I: Iterator<Item = T>;

impl<T, I> OptIter<T, I>
where
    I: Iterator<Item = T>,
{
    pub fn new(o: Option<I>) -> Self {
        Self(o)
    }
}

impl<'a, T, I> Iterator for OptIter<T, I>
where
    I: Iterator<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(I::next)
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

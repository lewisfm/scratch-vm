use std::{
    any::type_name,
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

#[repr(transparent)]
pub struct Id<T> {
    num: usize,
    phantom: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub const fn get(self) -> usize {
        self.num
    }
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            f.write_str("Id<")?;
            f.write_str(type_name::<T>())?;
            write!(f, ">({})", self.num)
        } else {
            f.debug_tuple("Id").field(&self.num).finish()
        }
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}

impl<T> From<usize> for Id<T> {
    fn from(value: usize) -> Self {
        Id {
            num: value,
            phantom: PhantomData,
        }
    }
}

impl<T> From<Id<T>> for usize {
    fn from(value: Id<T>) -> Self {
        value.num
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.num == other.num
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.num.cmp(&other.num)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.num.hash(state);
    }
}

/// A reference to a value that is _maybe_ mutable.
pub enum MaybeMutRef<'a, T> {
    Ref(&'a T),
    MutRef(&'a mut T),
}

impl<T> MaybeMutRef<'_, T> {
    /// Returns the mutable reference, if possible.
    #[inline]
    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Ref(_) => None,
            Self::MutRef(r) => Some(r),
        }
    }
}

impl<T> std::ops::Deref for MaybeMutRef<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Ref(r) => r,
            Self::MutRef(r) => r,
        }
    }
}

impl<T> AsRef<T> for MaybeMutRef<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        match self {
            Self::Ref(r) => r,
            Self::MutRef(r) => r,
        }
    }
}

#[test]
fn test_maybe_mut_ref() {
    {
        let x = 42;
        let mut x_ref = MaybeMutRef::Ref(&x);
        assert_eq!(x_ref.as_mut(), None);
        assert_eq!(*x_ref, 42);
    }
    {
        let mut x = 42;
        let mut x_ref = MaybeMutRef::MutRef(&mut x);
        assert_eq!(*x_ref, 42);
        *x_ref.as_mut().unwrap() = 1337;
        assert_eq!(*x_ref, 1337);
    }
}

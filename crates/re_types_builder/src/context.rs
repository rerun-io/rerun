use std::sync::mpsc;

/// Creates a new context.
///
/// The [`Context`] can be freely cloned and sent to other threads.
///
/// The [`ContextRoot`] should not be sent to other threads.
pub fn context() -> (ContextRoot, Context) {
    let (tx, rx) = mpsc::channel();
    (ContextRoot::new(rx), Context::new(tx))
}

#[derive(Clone)]
pub struct Context {
    errors: mpsc::Sender<anyhow::Error>,
}

impl Context {
    fn new(errors: mpsc::Sender<anyhow::Error>) -> Self {
        Self { errors }
    }

    pub fn error(&self, error: impl IntoError) {
        let _ = self.errors.send(error.into_error());
    }
}

pub struct ContextRoot {
    errors: mpsc::Receiver<anyhow::Error>,
    _not_send: std::marker::PhantomData<*mut ()>,
}

impl ContextRoot {
    fn new(errors: mpsc::Receiver<anyhow::Error>) -> Self {
        Self {
            errors,
            _not_send: std::marker::PhantomData,
        }
    }

    /// This outputs all errors to stderr and panics if there were any.
    pub fn panic_on_errors(&self) {
        let mut errored = false;

        while let Ok(err) = self.errors.try_recv() {
            errored = true;
            eprintln!("{err}");
        }

        #[allow(clippy::manual_assert)] // we don't want the noise of an assert
        if errored {
            panic!("Some errors occurred.");
        }
    }
}

const _: () = {
    trait IsNotSend<T> {
        fn __() {}
    }

    type False = ();

    struct True;

    struct Check<T: ?Sized>(T);

    impl<T: ?Sized> IsNotSend<True> for Check<T> {}

    impl<T: ?Sized + Send> IsNotSend<False> for Check<T> {}

    // if this fails with a type inference error,
    // then `ContextRoot` is `Send`, which it should _not_ be.
    let _ = <Check<ContextRoot> as IsNotSend<_>>::__;

    fn assert_send<T: Send>() {}
    let _ = assert_send::<Context>;
};

#[derive(Debug)]
struct Error {
    message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub trait IntoError {
    fn into_error(self) -> anyhow::Error;
}

impl IntoError for String {
    fn into_error(self) -> anyhow::Error {
        Error { message: self }.into()
    }
}

impl<'a> IntoError for &'a str {
    fn into_error(self) -> anyhow::Error {
        Error {
            message: self.into(),
        }
        .into()
    }
}

impl<'a> IntoError for std::borrow::Cow<'a, str> {
    fn into_error(self) -> anyhow::Error {
        Error {
            message: self.into(),
        }
        .into()
    }
}

impl IntoError for anyhow::Error {
    fn into_error(self) -> anyhow::Error {
        self
    }
}

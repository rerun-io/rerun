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

    pub fn error(&self, error: anyhow::Error) {
        let _ = self.errors.send(error);
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

    pub fn panic_if_errored(&self) {
        let mut errors = vec![];
        while let Ok(err) = self.errors.try_recv() {
            errors.push(err);
        }

        if errors.is_empty() {
            return;
        }

        for err in errors {
            eprintln!("{err}");
        }
        panic!("Some errors occurred.");
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

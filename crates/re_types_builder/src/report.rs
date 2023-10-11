use std::sync::mpsc;

/// Creates a new context.
///
/// The [`Reporter`] can be freely cloned and sent to other threads.
///
/// The [`Report`] should not be sent to other threads.
pub fn init() -> (Report, Reporter) {
    let (errors_tx, errors_rx) = mpsc::channel();
    let (warnings_tx, warnings_rx) = mpsc::channel();
    (
        Report::new(errors_rx, warnings_rx),
        Reporter::new(errors_tx, warnings_tx),
    )
}

/// Used to accumulate errors and warnings.
#[derive(Clone)]
pub struct Reporter {
    errors: mpsc::Sender<String>,
    warnings: mpsc::Sender<String>,
}

impl Reporter {
    fn new(errors: mpsc::Sender<String>, warnings: mpsc::Sender<String>) -> Self {
        Self { errors, warnings }
    }

    #[allow(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn error(&self, virtpath: &str, fqname: &str, text: impl ToString) {
        let _ = self
            .errors
            .send(format!("{virtpath} {fqname}: {}", text.to_string()));
    }

    #[allow(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn warn(&self, virtpath: &str, fqname: &str, text: impl ToString) {
        let _ = self
            .warnings
            .send(format!("{virtpath} {fqname}: {}", text.to_string()));
    }
}

/// Report which holds accumulated errors and warnings.
///
/// This should only exist on the main thread.
pub struct Report {
    errors: mpsc::Receiver<String>,
    warnings: mpsc::Receiver<String>,
    _not_send: std::marker::PhantomData<*mut ()>,
}

impl Report {
    fn new(errors: mpsc::Receiver<String>, warnings: mpsc::Receiver<String>) -> Self {
        Self {
            errors,
            warnings,
            _not_send: std::marker::PhantomData,
        }
    }

    /// This outputs all errors and warnings to stderr and panics if there were any errors.
    pub fn finalize(&self) {
        let mut errored = false;

        while let Ok(warn) = self.warnings.try_recv() {
            eprintln!("Warning: {warn}");
        }

        while let Ok(err) = self.errors.try_recv() {
            errored = true;
            eprintln!("Error: {err}");
        }

        if errored {
            println!("Some errors occurred.");
            std::process::exit(1);
        }
    }
}

const _: () = {
    // We want to ensure `Report` is `!Send`, so that it stays
    // on the main thread.
    //
    // This works by creating a type which has a different number of possible
    // implementations of a trait depending on its `Send`-ness:
    // - One impl for all `T: !Send`
    // - Two impls for all `T: Send`
    //
    // In an invocation like `Check<T> as IsNotSend<_>`, we're asking
    // the compiler to infer the type given to `IsNotSend` for us.
    // But if `Check<T>: Send`, then it has two possible implementations:
    // `IsNotSend<True>`, and `IsNotSend<False>`. This is a local ambiguity
    // and rustc has no way to disambiguate between the two implementations,
    // so it will instead output a type inference error.
    //
    // We could use `static_assertions` here, but we'd rather not pay the
    // compilation cost for a single invocation. Some of the macros from
    // that crate are pretty gnarly!
    trait IsNotSend<T> {
        fn __() {}
    }

    type False = ();

    struct True;

    struct Check<T: ?Sized>(T);

    impl<T: ?Sized> IsNotSend<True> for Check<T> {}

    impl<T: ?Sized + Send> IsNotSend<False> for Check<T> {}

    // if this fails with a type inference error,
    // then `Report` is `Send`, which it should _not_ be.
    let _ = <Check<Report> as IsNotSend<_>>::__;

    fn assert_send<T: Send>() {}
    let _ = assert_send::<Reporter>;
};

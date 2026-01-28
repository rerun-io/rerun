use camino::Utf8Path;
use crossbeam::channel::{Receiver, Sender};

/// Creates a new context.
///
/// The [`Reporter`] can be freely cloned and sent to other threads.
///
/// The [`Report`] should not be sent to other threads.
pub fn init() -> (Report, Reporter) {
    let (errors_tx, errors_rx) = crossbeam::channel::bounded(1024);
    let (warnings_tx, warnings_rx) = crossbeam::channel::bounded(1024);
    (
        Report::new(errors_rx, warnings_rx),
        Reporter::new(errors_tx, warnings_tx),
    )
}

/// Used to accumulate errors and warnings.
#[derive(Clone)]
pub struct Reporter {
    errors: Sender<String>,
    warnings: Sender<String>,
}

impl Reporter {
    fn new(errors: Sender<String>, warnings: Sender<String>) -> Self {
        Self { errors, warnings }
    }

    /// Error about a file as a whole.
    ///
    /// Use sparingly for things like failing to write a file or failing to format it.
    #[expect(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn error_file(&self, path: &Utf8Path, text: impl ToString) {
        self.errors
            .send(format!("{path}: {}", text.to_string()))
            .ok();
    }

    #[expect(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn error(&self, virtpath: &str, fqname: &str, text: impl ToString) {
        self.errors
            .send(format!(
                "{} {fqname}: {}",
                Self::format_virtpath(virtpath),
                text.to_string()
            ))
            .ok();
    }

    #[expect(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn warn_no_context(&self, text: impl ToString) {
        self.warnings.send(text.to_string()).ok();
    }

    #[expect(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn warn(&self, virtpath: &str, fqname: &str, text: impl ToString) {
        self.warnings
            .send(format!(
                "{} {fqname}: {}",
                Self::format_virtpath(virtpath),
                text.to_string()
            ))
            .ok();
    }

    #[expect(clippy::needless_pass_by_value)] // `&impl ToString` has worse usability
    pub fn error_any(&self, text: impl ToString) {
        self.errors.send(text.to_string()).ok();
    }

    // Tries to format a virtual fbs path such that it can be clicked in the CLI.
    fn format_virtpath(virtpath: &str) -> String {
        if let Ok(path) = Utf8Path::new(virtpath).canonicalize() {
            path.display().to_string()
        } else if let Ok(path) =
            Utf8Path::new(&format!("crates/store/re_sdk_types/definitions/{virtpath}"))
                .canonicalize()
        {
            path.display().to_string()
        } else {
            virtpath.to_owned()
        }
    }
}

/// Report which holds accumulated errors and warnings.
///
/// This should only exist on the main thread.
pub struct Report {
    errors: Receiver<String>,
    warnings: Receiver<String>,
    _not_send: std::marker::PhantomData<*mut ()>,
}

impl Report {
    fn new(errors: Receiver<String>, warnings: Receiver<String>) -> Self {
        Self {
            errors,
            warnings,
            _not_send: std::marker::PhantomData,
        }
    }

    /// This outputs all errors and warnings to stderr and panics if there were any errors.
    pub fn finalize(&self, warnings_as_errors: bool) {
        use colored::Colorize as _;

        let mut any_errors = false;

        while let Ok(warn) = self.warnings.try_recv() {
            if warnings_as_errors {
                any_errors = true;
                eprintln!(
                    "{} {}",
                    "Error (warnings as errors enabled): ".red().bold(),
                    warn
                );
            } else {
                eprintln!("{} {}", "Warning: ".yellow().bold(), warn);
            }
        }

        while let Ok(err) = self.errors.try_recv() {
            any_errors = true;
            eprintln!("{} {}", "Error: ".red().bold(), err);
        }

        if any_errors {
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

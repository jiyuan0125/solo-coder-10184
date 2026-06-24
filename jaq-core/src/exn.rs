//! Exceptions and errors.

use crate::{compile::TermId, filter::Vars, RcList};
use alloc::{boxed::Box, string::String, string::ToString, vec::Vec};
use core::fmt::{self, Display};

/// Exception.
///
/// This is either an error, a runtime halt, or control flow data internal to jaq.
/// Users should only be able to observe the first two cases.
///
/// Use [`crate::unwrap_valr`] to convert a [`crate::ValX`] to an error.
#[derive(Clone, Debug)]
pub struct Exn<'a, V>(pub(crate) Inner<'a, V>);

#[derive(Clone, Debug)]
pub(crate) enum Inner<'a, V> {
    Err(Box<Error<V>>),
    /// Tail-recursive call.
    ///
    /// This is used internally to execute tail-recursive filters.
    /// If this can be observed by users, then this is a bug.
    TailCall(Box<(&'a TermId, Vars<V>, CallInput<V>)>),
    Break(usize),
    Halt(i32),
}

#[derive(Clone, Debug)]
pub(crate) enum CallInput<V> {
    Run(V),
    Paths((V, RcList<V>)),
}

impl<V> CallInput<V> {
    pub fn unwrap_run(self) -> V {
        match self {
            Self::Run(v) => v,
            _ => panic!(),
        }
    }

    pub fn unwrap_paths(self) -> (V, RcList<V>) {
        match self {
            Self::Paths(vp) => vp,
            _ => panic!(),
        }
    }
}

impl<V> Exn<'_, V> {
    /// If the exception is an error, yield it, else yield the exception.
    pub fn get_err(self) -> Result<Error<V>, Self> {
        match self.0 {
            Inner::Err(e) => Ok(*e),
            _ => Err(self),
        }
    }

    /// If the exception halts, yield the exit code, else yield the exception.
    pub fn get_halt(self) -> Result<i32, Self> {
        match self.0 {
            Inner::Halt(code) => Ok(code),
            _ => Err(self),
        }
    }

    /// Convert any exception into an error.
    ///
    /// Unlike [`unwrap_valr`](crate::unwrap_valr), this function does not
    /// exit the process or panic on halt exceptions.
    /// Instead, halt exceptions are converted into regular errors.
    ///
    /// This is useful for library users who want to handle all exceptions
    /// gracefully without terminating the process.
    pub fn into_err(self) -> Error<V>
    where
        V: From<alloc::string::String>,
    {
        match self.0 {
            Inner::Err(e) => *e,
            Inner::Halt(code) => Error::str(alloc::format!("halt: {}", code)),
            Inner::Break(_) => Error::str("break (internal)"),
            Inner::TailCall(_) => Error::str("tail call (internal)"),
        }
    }

    /// Create an exception intended to halt filter execution.
    ///
    /// This is used by the `halt/1` filter.
    pub fn halt(exit_code: i32) -> Self {
        Self(Inner::Halt(exit_code))
    }
}

impl<V> From<Error<V>> for Exn<'_, V> {
    fn from(e: Error<V>) -> Self {
        Exn(Inner::Err(Box::new(e)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part<V, S = &'static str> {
    Val(V),
    Str(S),
}

/// Error that occurred during filter execution.
///
/// An error has two parts:
/// - The **core** error message, which determines equality and
///   the value produced by `into_val()`. This is what program logic
///   (such as `try/catch`) operates on.
/// - An optional **context stack**, which is only used for display
///   to the user and does not affect equality or `into_val()`.
#[derive(Clone, Debug)]
pub struct Error<V> {
    core: Part<V, Vec<Part<V>>>,
    ctx: Vec<&'static str>,
}

impl<V: PartialEq> PartialEq for Error<V> {
    fn eq(&self, other: &Self) -> bool {
        // Context is intentionally excluded from equality comparison.
        // Two errors that mean the same thing should be considered equal
        // regardless of where in the pipeline they originated.
        self.core == other.core
    }
}

impl<V: Eq> Eq for Error<V> {}

impl<V> Error<V> {
    /// Create a new error from a value.
    pub fn new(v: V) -> Self {
        Self {
            core: Part::Val(v),
            ctx: Vec::new(),
        }
    }

    /// Create a path expression error.
    pub fn path_expr(v: V) -> Self {
        Self {
            core: Part::Str(Vec::from([
                Part::Str("invalid path expression with input "),
                Part::Val(v),
            ])),
            ctx: Vec::new(),
        }
    }

    /// Create a type error.
    pub fn typ(v: V, typ: &'static str) -> Self {
        use Part::{Str, Val};
        let err: Self = [Str("cannot use "), Val(v), Str(" as "), Str(typ)]
            .into_iter()
            .collect();
        err
    }

    /// Create a math error.
    pub fn math(l: V, op: crate::ops::Math, r: V) -> Self {
        use Part::{Str, Val};
        let err: Self = [
            Str("cannot calculate "),
            Val(l),
            Str(" "),
            Str(op.as_str()),
            Str(" "),
            Val(r),
        ]
        .into_iter()
        .collect();
        err
    }

    /// Create an indexing error.
    pub fn index(l: V, r: V) -> Self {
        use Part::{Str, Val};
        let err: Self = [Str("cannot index "), Val(l), Str(" with "), Val(r)]
            .into_iter()
            .collect();
        err
    }
}

impl<V: From<String>> Error<V> {
    /// Build an error from something that can be converted to a string.
    pub fn str(s: impl ToString) -> Self {
        Self {
            core: Part::Val(V::from(s.to_string())),
            ctx: Vec::new(),
        }
    }

    /// Prepend context information to an existing error.
    ///
    /// This is used to give users a hint about where in a complex filter
    /// pipeline an error occurred, without changing the semantics of
    /// whether an error is raised.
    ///
    /// Context is only used for display; it does not affect `PartialEq`
    /// or `into_val()`.
    pub fn with_ctx(mut self, ctx: &'static str) -> Self {
        self.ctx.push(ctx);
        self
    }
}

impl<'a, V> Exn<'a, V> {
    /// Prepend context information to an error exception.
    ///
    /// Non-error exceptions (halt, break, tail call) are passed through unchanged.
    pub fn with_err_ctx(self, ctx: &'static str) -> Self
    where
        V: From<String>,
    {
        match self.0 {
            Inner::Err(e) => Exn(Inner::Err(Box::new(e.with_ctx(ctx)))),
            other => Exn(other),
        }
    }
}

impl<V> FromIterator<Part<V>> for Error<V> {
    fn from_iter<T: IntoIterator<Item = Part<V>>>(iter: T) -> Self {
        Self {
            core: Part::Str(iter.into_iter().collect()),
            ctx: Vec::new(),
        }
    }
}

impl<V: From<String> + Display> Error<V> {
    /// Convert the error into a value to be used by `catch` filters.
    ///
    /// This intentionally returns only the core error message without
    /// the context stack, so that program logic comparing error values
    /// is not affected by where in the pipeline the error occurred.
    pub fn into_val(self) -> V {
        if let Part::Val(v) = self.core {
            v
        } else {
            let core_msg: String = match &self.core {
                Part::Val(v) => alloc::format!("{}", v),
                Part::Str(parts) => {
                    let mut s = String::new();
                    for part in parts {
                        match part {
                            Part::Val(v) => s.push_str(&alloc::format!("{}", v)),
                            Part::Str(t) => s.push_str(t),
                        }
                    }
                    s
                }
            };
            V::from(core_msg)
        }
    }
}

impl<V: Display> Display for Error<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Context is printed outermost-first (reverse of push order)
        for ctx in self.ctx.iter().rev() {
            write!(f, "{}: ", ctx)?;
        }
        match &self.core {
            Part::Val(v) => v.fmt(f),
            Part::Str(parts) => parts.iter().try_for_each(|part| match part {
                Part::Val(v) => v.fmt(f),
                Part::Str(s) => s.fmt(f),
            }),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use alloc::string::String;
    use std::panic::catch_unwind;

    #[test]
    fn exn_into_err_converts_all_types() {
        // Halt exception should convert to error without panicking
        let halt: Exn<'_, String> = Exn::halt(42);
        let err = halt.into_err();
        assert!(err.to_string().contains("halt: 42"));

        // Regular error should pass through
        let err_val: Error<String> = Error::str("test error");
        let exn: Exn<'_, String> = Exn::from(err_val);
        let err = exn.into_err();
        assert_eq!(err.to_string(), "test error");
    }

    #[test]
    fn halt_does_not_exit_process_via_into_err() {
        // Verify that into_err() does not call process::exit
        let halt: Exn<'_, String> = Exn::halt(1);
        let result = catch_unwind(|| {
            halt.into_err()
        });
        assert!(result.is_ok());
    }
}

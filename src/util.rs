/// Format an error and its full cause chain into a [`Formatter`](std::fmt::Formatter).
///
/// Useful for implementing [`std::fmt::Display`] on custom error types that wrap
/// other errors — each cause is printed on its own line with a `"Caused by:"` prefix.
///
/// # Examples
///
/// ```
/// use mae::util::error_chain_fmt;
/// use std::fmt;
///
/// #[derive(Debug)]
/// struct Inner;
/// impl fmt::Display for Inner {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "inner") }
/// }
/// impl std::error::Error for Inner {}
///
/// #[derive(Debug)]
/// struct Outer(Inner);
/// impl fmt::Display for Outer {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         error_chain_fmt(&self.0, f)
///     }
/// }
/// impl std::error::Error for Outer {
///     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.0) }
/// }
///
/// let err = Outer(Inner);
/// let s = format!("{}", err);
/// assert!(s.contains("inner"));
/// ```
pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_,>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause,) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok((),)
}

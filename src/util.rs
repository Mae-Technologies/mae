use std::collections::HashMap;

/// Build a URL query suffix from a param map.
///
/// Returns an empty string for an empty map, otherwise `"?k=v&..."`.
pub fn build_query_string(q: &HashMap<String, String>) -> String {
    if q.is_empty() {
        return String::new();
    }
    format!("?{}", q.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"))
}

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
    f: &mut std::fmt::Formatter<'_>
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::must::*;

    #[test]
    fn build_query_string_empty_returns_empty() {
        let q = HashMap::new();
        must_eq(build_query_string(&q).as_str(), "");
    }

    #[test]
    fn build_query_string_single_param() {
        let mut q = HashMap::new();
        q.insert("sys_client".to_string(), "5".to_string());
        let result = build_query_string(&q);
        must_eq(result.as_str(), "?sys_client=5");
    }

    #[test]
    fn build_query_string_multiple_params() {
        let mut q = HashMap::new();
        q.insert("a".to_string(), "1".to_string());
        q.insert("b".to_string(), "2".to_string());
        let result = build_query_string(&q);
        must_be_true(result.starts_with('?'));
        must_be_true(result.contains("a=1"));
        must_be_true(result.contains("b=2"));
    }
}

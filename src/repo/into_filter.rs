use super::map_util::Filter;

/// Converts a typed value into a [`Filter`] condition.
///
/// Implemented for the primitive types that appear in schema field
/// definitions (`i32`, `String`, `Option<i32>`, `Option<String>`).
/// The generated `From<PatchField> for FilterOp<Field>` and
/// `From<UpdateRow> for Vec<FilterOp<Field>>` impls produced by
/// `#[derive(MaeRepo)]` rely on this trait to pick the correct
/// [`Filter`] variant for each field type.
pub trait IntoMaeFilter {
    /// Convert `self` into a [`Filter`] using an equality /
    /// string-equality condition appropriate for the value's type.
    fn into_mae_filter(self) -> Filter;
}

impl IntoMaeFilter for i32 {
    fn into_mae_filter(self) -> Filter {
        Filter::Equals(self)
    }
}

impl IntoMaeFilter for String {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self)
    }
}

impl IntoMaeFilter for Option<i32> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => Filter::Equals(v),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for Option<String> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => Filter::StringIs(v),
            None => Filter::IsNull
        }
    }
}

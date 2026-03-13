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

impl IntoMaeFilter for crate::repo::default::DomainStatus {
    fn into_mae_filter(self) -> Filter {
        let v = match self {
            crate::repo::default::DomainStatus::Incomplete => "incomplete",
            crate::repo::default::DomainStatus::Active => "active",
            crate::repo::default::DomainStatus::Deleted => "deleted",
            crate::repo::default::DomainStatus::Archived => "archived"
        };
        Filter::StringIs(v.to_string())
    }
}

impl IntoMaeFilter for Option<crate::repo::default::DomainStatus> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for serde_json::Value {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

impl IntoMaeFilter for Option<serde_json::Value> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => Filter::StringIs(v.to_string()),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for bool {
    fn into_mae_filter(self) -> Filter {
        Filter::Equals(if self { 1 } else { 0 })
    }
}

impl IntoMaeFilter for Option<bool> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for chrono::NaiveDate {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

impl IntoMaeFilter for Option<chrono::NaiveDate> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

#[cfg(feature = "decimal")]
impl IntoMaeFilter for rust_decimal::Decimal {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

#[cfg(feature = "decimal")]
impl IntoMaeFilter for Option<rust_decimal::Decimal> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

#[cfg(feature = "uuid")]
impl IntoMaeFilter for uuid::Uuid {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

#[cfg(feature = "uuid")]
impl IntoMaeFilter for Option<uuid::Uuid> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

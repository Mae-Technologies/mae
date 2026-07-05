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

impl IntoMaeFilter for chrono::DateTime<chrono::Utc> {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

impl IntoMaeFilter for Option<chrono::DateTime<chrono::Utc>> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for rust_decimal::Decimal {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

impl IntoMaeFilter for Option<rust_decimal::Decimal> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

impl IntoMaeFilter for uuid::Uuid {
    fn into_mae_filter(self) -> Filter {
        Filter::StringIs(self.to_string())
    }
}

impl IntoMaeFilter for Option<uuid::Uuid> {
    fn into_mae_filter(self) -> Filter {
        match self {
            Some(v) => v.into_mae_filter(),
            None => Filter::IsNull
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::default::DomainStatus;
    use crate::testing::must::must_eq;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn i32_maps_to_equals() {
        match 7.into_mae_filter() {
            Filter::Equals(v) => must_eq(v, 7),
            other => panic!("unexpected filter: {other:?}")
        }
    }

    #[test]
    fn string_maps_to_string_is() {
        match "hello".to_string().into_mae_filter() {
            Filter::StringIs(v) => must_eq(v.as_str(), "hello"),
            other => panic!("unexpected filter: {other:?}")
        }
    }

    #[test]
    fn option_i32_none_is_null() {
        let none: Option<i32> = None;
        assert!(matches!(none.into_mae_filter(), Filter::IsNull));
    }

    #[test]
    fn option_string_and_status_none_is_null() {
        let none_str: Option<String> = None;
        assert!(matches!(none_str.into_mae_filter(), Filter::IsNull));

        let none_status: Option<DomainStatus> = None;
        assert!(matches!(none_status.into_mae_filter(), Filter::IsNull));

        let none_bool: Option<bool> = None;
        assert!(matches!(none_bool.into_mae_filter(), Filter::IsNull));
    }

    #[test]
    fn bool_maps_to_one_or_zero() {
        match true.into_mae_filter() {
            Filter::Equals(v) => must_eq(v, 1),
            other => panic!("unexpected filter: {other:?}")
        }
        match false.into_mae_filter() {
            Filter::Equals(v) => must_eq(v, 0),
            other => panic!("unexpected filter: {other:?}")
        }
    }

    #[test]
    fn domain_status_maps_to_string_is() {
        match DomainStatus::Active.into_mae_filter() {
            Filter::StringIs(v) => must_eq(v.as_str(), "active"),
            other => panic!("unexpected filter: {other:?}")
        }
    }

    #[test]
    fn json_value_serializes_to_string() {
        let value = serde_json::json!({"a": 1});
        match value.into_mae_filter() {
            Filter::StringIs(v) => must_eq(v.contains("a"), true),
            other => panic!("unexpected filter: {other:?}")
        }
    }

    #[test]
    fn naive_date_and_datetime_map_to_string() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).expect("date");
        assert!(matches!(date.into_mae_filter(), Filter::StringIs(_)));

        let dt = Utc::now();
        assert!(matches!(dt.into_mae_filter(), Filter::StringIs(_)));
    }

    #[test]
    fn decimal_and_uuid_map_to_string() {
        let dec = Decimal::new(42, 0);
        assert!(matches!(dec.into_mae_filter(), Filter::StringIs(_)));

        let id = Uuid::nil();
        match id.into_mae_filter() {
            Filter::StringIs(v) => must_eq(v.as_str(), id.to_string().as_str()),
            other => panic!("unexpected filter: {other:?}")
        }
    }
}

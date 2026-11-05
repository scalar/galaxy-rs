//! Three-state serialization for optional, nullable request fields.
//!
//! A field the API lets you omit AND set to `null` renders as
//! `Option<Option<T>>`:
//!
//! - `None` leaves the key out of the body entirely;
//! - `Some(None)` sends `"field": null`, which is how a PATCH clears a value;
//! - `Some(Some(value))` sends the value.
//!
//! A plain `Option<T>` with `skip_serializing_if` can only express the first and
//! last. serde's stock `Option<Option<T>>` decoding cannot tell an absent key from
//! `null` either, so the field also needs the reader below: an absent key falls
//! back to `#[serde(default)]` (`None`), and a present one, `null` included, is
//! wrapped in `Some`.
//!
//! Wired in per field via `#[serde(with = "crate::explicit_null::double_option")]`
//! next to `default` and `skip_serializing_if = "Option::is_none"`; callers see
//! the plain nested `Option`, never this module.

/// The generic `with` pair for an `Option<Option<T>>` field.
pub mod double_option {
    use serde::{Deserialize as _, Serialize as _};

    /// Writes `Some(None)` as `null` and `Some(Some(value))` as the value.
    ///
    /// `None` never reaches here: the field's `skip_serializing_if` drops the key
    /// first. It still writes `null` rather than panicking, so a caller that
    /// serializes the field on its own gets the same answer as `Some(None)`.
    pub fn serialize<T, S>(value: &Option<Option<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: serde::Serialize,
        S: serde::Serializer,
    {
        match value {
            Some(inner) => inner.serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    /// Reads a present key, `null` included, as `Some(_)`.
    ///
    /// Only ever called for a key that is present; an absent one takes the
    /// field's `#[serde(default)]` and stays `None`.
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: serde::Deserialize<'de>,
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

#[cfg(test)]
mod tests {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Patch {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "crate::explicit_null::double_option"
        )]
        name: Option<Option<String>>,
    }

    #[test]
    fn each_state_has_its_own_wire_form() {
        let encode = |name| serde_json::to_string(&Patch { name }).expect("encodes");
        assert_eq!(encode(None), "{}");
        assert_eq!(encode(Some(None)), r#"{"name":null}"#);
        assert_eq!(encode(Some(Some("a".to_string()))), r#"{"name":"a"}"#);
    }

    #[test]
    fn each_wire_form_decodes_back_to_its_own_state() {
        let decode = |raw: &str| serde_json::from_str::<Patch>(raw).expect("decodes").name;
        assert_eq!(decode("{}"), None);
        assert_eq!(decode(r#"{"name":null}"#), Some(None));
        assert_eq!(decode(r#"{"name":"a"}"#), Some(Some("a".to_string())));
    }
}

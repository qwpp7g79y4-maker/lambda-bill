//! A budget for the JSON values of one document.
//!
//! `serde_json::Value` costs about 100 bytes for each scalar and about 300 for
//! each small array, so the size of the text says little about the memory a
//! document needs. A counting pass reads the text without building anything and
//! stops at the budget, before the real parse allocates.

use std::cell::Cell;
use std::fmt;

use serde::de::{
    DeserializeSeed, Deserializer, Error as DeError, IgnoredAny, MapAccess, SeqAccess, Visitor,
};

/// The most JSON values one document may have. A scalar and an object key
/// count 1, an array or object counts 4.
pub(crate) const MAX_JSON_UNITS: u64 = 4_000_000;

/// The key `serde_json` uses for a number when `arbitrary_precision` is on.
const NUMBER_KEY: &str = "$serde_json::private::Number";

/// Whether the text, when it is valid JSON, stays within the budget. Text that
/// is not JSON is `true`: the real parse reports it.
pub(crate) fn within_budget(text: &str) -> bool {
    within(text, MAX_JSON_UNITS)
}

fn within(text: &str, limit: u64) -> bool {
    let counter = Counter {
        spent: Cell::new(0),
        over: Cell::new(false),
        limit,
    };
    let mut de = serde_json::Deserializer::from_str(text);
    let _ = (&counter).deserialize(&mut de);
    !counter.over.get()
}

struct Counter {
    spent: Cell<u64>,
    over: Cell<bool>,
    limit: u64,
}

impl Counter {
    fn spend<E: DeError>(&self, units: u64) -> Result<(), E> {
        let total = self.spent.get() + units;
        self.spent.set(total);
        if total > self.limit {
            self.over.set(true);
            return Err(E::custom("too many values"));
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for &Counter {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for &Counter {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_bool<E: DeError>(self, _: bool) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_i64<E: DeError>(self, _: i64) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_u64<E: DeError>(self, _: u64) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_f64<E: DeError>(self, _: f64) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_str<E: DeError>(self, _: &str) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_unit<E: DeError>(self) -> Result<(), E> {
        self.spend(1)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.spend(4)?;
        while seq.next_element_seed(self)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut first = true;
        while let Some(key) = map.next_key::<String>()? {
            if first && key == NUMBER_KEY {
                // A number: one scalar, not an object.
                map.next_value::<IgnoredAny>()?;
                return self.spend(1);
            }
            if first {
                self.spend(4)?;
                first = false;
            }
            self.spend(1)?;
            map.next_value_seed(self)?;
        }
        if first {
            self.spend(4)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::within;

    #[test]
    fn scalars_arrays_objects_and_keys_are_counted() {
        // [1,2,3] is 4 + 3.
        assert!(within("[1,2,3]", 7));
        assert!(!within("[1,2,3]", 6));
        // {"a":1} is 4 + 1 + 1.
        assert!(within("{\"a\":1}", 6));
        assert!(!within("{\"a\":1}", 5));
        // [] and {} are 4 each.
        assert!(within("[[],{}]", 12));
        assert!(!within("[[],{}]", 11));
    }

    #[test]
    fn a_number_with_arbitrary_precision_is_one_unit() {
        assert!(within("[1.5,12345678901234567890123,1e5]", 7));
        assert!(!within("[1.5,12345678901234567890123,1e5]", 6));
    }

    #[test]
    fn text_that_is_not_json_is_left_to_the_real_parse() {
        assert!(within("[2026-11-19T10:00:00Z] REPORT RequestId: a", 100));
        assert!(within("{'a': 1}", 100));
        assert!(within("[1,2", 100));
    }
}

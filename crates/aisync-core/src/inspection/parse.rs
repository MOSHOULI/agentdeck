//! Bounded strict JSON; unsupported syntax is not proof of invalid Qwen config.
use super::{InspectionIssue, LIMITS};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;
type Result<T> = std::result::Result<T, InspectionIssue>;
#[derive(Default)]
struct Budget {
    entries: usize,
    failure: Option<InspectionIssue>,
}

impl Budget {
    fn fail<E: de::Error>(&mut self, issue: InspectionIssue) -> E {
        self.failure = Some(issue);
        E::custom("manifest rejected")
    }
    fn entry<E: de::Error>(&mut self) -> std::result::Result<(), E> {
        self.entries += 1;
        if self.entries > LIMITS.max_json_entries {
            Err(self.fail(InspectionIssue::JsonLimit))
        } else {
            Ok(())
        }
    }
    fn string<E: de::Error>(&mut self, value: &str) -> std::result::Result<(), E> {
        if value.len() > LIMITS.max_json_string_bytes {
            Err(self.fail(InspectionIssue::JsonLimit))
        } else {
            Ok(())
        }
    }
}

struct StrictValue<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictValue<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        self.budget.entry()?;
        if self.depth > LIMITS.max_json_depth {
            return Err(self.budget.fail(InspectionIssue::JsonLimit));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictValue<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Value, E> {
        self.budget.string(value)?;
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Value, E> {
        self.budget.string(&value)?;
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(StrictValue {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget.entry()?;
            self.budget.string(&key)?;
            if values.contains_key(&key) {
                return Err(self.budget.fail(InspectionIssue::DuplicateJsonKey));
            }
            let value = map.next_value_seed(StrictValue {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

pub(crate) fn parse_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > LIMITS.max_manifest_bytes {
        return Err(InspectionIssue::FileTooLarge);
    }
    let mut budget = Budget::default();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = StrictValue {
        budget: &mut budget,
        depth: 1,
    }
    .deserialize(&mut deserializer);
    let value = result.map_err(|_| budget.failure.unwrap_or(InspectionIssue::UnsupportedJson))?;
    deserializer
        .end()
        .map_err(|_| InspectionIssue::UnsupportedJson)?;
    Ok(value)
}

//! Strict bounded JSON, followed by a deliberately small output allowlist.
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::{
    DeclaredDependency, DependencySection, InspectionIssue, LIMITS, ProfileMetadata,
    RuntimeMetadata, SpecificationKind, UnknownState,
};

type Result<T> = std::result::Result<T, InspectionIssue>;

pub(super) fn parse_profile(bytes: &[u8]) -> Result<ProfileMetadata> {
    let value = parse_json(bytes)?;
    let object = object(&value)?;
    let dsh = manifest_extension(object)?;
    let name = optional_string(object, "name")?
        .map(validated_name)
        .transpose()?;
    let mut declared_dependencies = Vec::new();
    for (field, section) in [
        ("dependencies", DependencySection::Dependencies),
        (
            "optionalDependencies",
            DependencySection::OptionalDependencies,
        ),
        ("peerDependencies", DependencySection::PeerDependencies),
    ] {
        if let Some(dependencies) = object.get(field) {
            let dependencies = self::object(dependencies)?;
            if declared_dependencies.len() + dependencies.len() > LIMITS.max_dependencies {
                return Err(InspectionIssue::DependencyLimit);
            }
            for (name, specification) in dependencies {
                let name = validated_name(name)?;
                let specification = specification
                    .as_str()
                    .ok_or(InspectionIssue::UnsupportedShape)?;
                let specification_kind = if is_numeric_range(specification) {
                    SpecificationKind::RegistryVersionOrRange
                } else {
                    SpecificationKind::OtherRedacted
                };
                declared_dependencies.push(DeclaredDependency {
                    name,
                    section,
                    specification_kind,
                    installed: UnknownState::Unknown,
                    actual_enabled: UnknownState::Unknown,
                });
            }
        }
    }
    let mut saved_bundle_selections = Vec::new();
    if let Some(profile) = dsh.and_then(|dsh| dsh.get("profile"))
        && let Some(bundles) = self::object(profile)?.get("bundles")
    {
        let bundles = bundles
            .as_array()
            .ok_or(InspectionIssue::UnsupportedShape)?;
        if bundles.len() > LIMITS.max_saved_bundles {
            return Err(InspectionIssue::BundleLimit);
        }
        for bundle in bundles {
            saved_bundle_selections.push(validated_name(
                bundle.as_str().ok_or(InspectionIssue::UnsupportedShape)?,
            )?);
        }
    }
    Ok(ProfileMetadata {
        name,
        declared_dependencies,
        saved_bundle_selections,
    })
}

pub(super) fn parse_runtime(bytes: &[u8]) -> Result<RuntimeMetadata> {
    let value = parse_json(bytes)?;
    let object = object(&value)?;
    manifest_extension(object)?;
    let name = optional_string(object, "name")?
        .map(validated_name)
        .transpose()?;
    let reported_version = optional_string(object, "version")?
        .map(|version| {
            if is_version(version) {
                Ok(version.to_owned())
            } else {
                Err(InspectionIssue::UnsupportedMetadata)
            }
        })
        .transpose()?;
    let declared_node_engine = if let Some(engines) = object.get("engines") {
        optional_string(self::object(engines)?, "node")?
            .map(|engine| {
                if is_numeric_range(engine) {
                    Ok(engine.to_owned())
                } else {
                    Err(InspectionIssue::UnsupportedMetadata)
                }
            })
            .transpose()?
    } else {
        None
    };
    Ok(RuntimeMetadata {
        name,
        reported_version,
        declared_node_engine,
    })
}

fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or(InspectionIssue::UnsupportedShape)
}

fn optional_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<Option<&'a str>> {
    object
        .get(field)
        .map(|value| value.as_str().ok_or(InspectionIssue::UnsupportedShape))
        .transpose()
}

fn manifest_extension(object: &Map<String, Value>) -> Result<Option<&Map<String, Value>>> {
    let Some(dsh) = object.get("dsh") else {
        return Ok(None);
    };
    let dsh = self::object(dsh)?;
    if let Some(version) = dsh.get("manifestVersion")
        && version.as_u64() != Some(1)
    {
        return Err(InspectionIssue::UnsupportedManifestVersion);
    }
    Ok(Some(dsh))
}

fn validated_name(name: &str) -> Result<String> {
    fn part(value: &str) -> bool {
        !value.is_empty()
            && value.as_bytes()[0].is_ascii_alphanumeric()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b))
    }
    let valid = if let Some(scoped) = name.strip_prefix('@') {
        scoped
            .split_once('/')
            .is_some_and(|(scope, package)| part(scope) && part(package))
    } else {
        part(name)
    };
    if !valid || name.len() > LIMITS.max_name_bytes {
        return Err(InspectionIssue::UnsupportedMetadata);
    }
    Ok(name.to_owned())
}

fn is_version(version: &str) -> bool {
    static VERSION: OnceLock<Regex> = OnceLock::new();
    version.len() <= LIMITS.max_version_bytes
        && VERSION.get_or_init(|| Regex::new(
            r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$"
        ).expect("constant version expression")).is_match(version)
}

/// Bounded numeric-pattern classification, not full npm semver validation or
/// resolution. Unrecognized specs are redacted; no raw URL, path or tag is emitted.
fn is_numeric_range(range: &str) -> bool {
    static RANGE: OnceLock<Regex> = OnceLock::new();
    !range.is_empty()
        && range.len() <= LIMITS.max_version_bytes
        && RANGE.get_or_init(|| Regex::new(
            r"^(\*|[~^<>=]*[0-9]+(\.[0-9xX*]+){0,2}(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?)( (\|\| )?(\*|[~^<>=]*[0-9]+(\.[0-9xX*]+){0,2}(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?))*$"
        ).expect("constant range expression")).is_match(range)
}

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

fn parse_json(bytes: &[u8]) -> Result<Value> {
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
    let value = result.map_err(|_| budget.failure.unwrap_or(InspectionIssue::InvalidJson))?;
    deserializer
        .end()
        .map_err(|_| InspectionIssue::InvalidJson)?;
    Ok(value)
}

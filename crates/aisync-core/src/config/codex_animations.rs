//! Pure in-memory protocol for one reviewed Codex display preference.
//! No filesystem, environment, agent execution, IPC or persistence here.
//! `binding` is an opaque caller selection identity, NOT a validated path.
//! Receipt digests detect changed images; they do not authenticate receipt authors.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toml_edit::{Document, Item, Value};

pub const SCHEMA_COMMIT: &str = "6dfaca4dad030f5ecaf4925f210f539353b15594";
pub const FIELD: &str = "tui.animations";
const MAX_BYTES: usize = 256 * 1024;
const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 32;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error(
        "Unsupported Codex animations document or protocol input; no configuration values are reported"
    )]
    Unsupported,
    #[error("Codex animations preview is stale or belongs to another selection")]
    Changed,
    #[error("Codex animations document differs from the exact recorded image")]
    Conflict,
}
type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub field: &'static str,
    pub before: bool,
    pub after: bool,
    pub changes: bool,
    pub token: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    Applied,
    Restoring,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    format: u8,
    schema_commit: String,
    field: String,
    phase: Phase,
    binding_digest: String,
    before: bool,
    after: bool,
    before_digest: String,
    after_digest: String,
    restore_digest: Option<String>,
}
impl Receipt {
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
}

/// Contents are sensitive caller-owned memory; never serialize or debug-print.
pub struct PreparedChange {
    pub output: Vec<u8>,
    pub receipt: Receipt,
}
impl std::fmt::Debug for PreparedChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedChange")
            .field("output", &"[redacted]")
            .field("receipt", &self.receipt)
            .finish()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryAction {
    RestoreOriginal,
    CleanupReceipt,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn binding_digest(binding: &[u8]) -> Result<String> {
    if binding.is_empty() || binding.len() > 4096 {
        return Err(ProtocolError::Unsupported);
    }
    let mut h = Sha256::new();
    h.update(b"agentdeck/codex-animations/selection/1\0");
    h.update(binding);
    Ok(format!("{:x}", h.finalize()))
}
fn valid_digest(d: &str) -> bool {
    d.len() == 64
        && d.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_receipt(r: &Receipt) -> Result<()> {
    if r.format != 1
        || r.schema_commit != SCHEMA_COMMIT
        || r.field != FIELD
        || r.before == r.after
        || ![&r.binding_digest, &r.before_digest, &r.after_digest]
            .iter()
            .all(|d| valid_digest(d))
        || r.before_digest == r.after_digest
        || match r.phase {
            Phase::Restoring => r.restore_digest.as_ref() != Some(&r.before_digest),
            _ => r.restore_digest.is_some(),
        }
    {
        return Err(ProtocolError::Unsupported);
    }
    Ok(())
}
pub fn decode_receipt(bytes: &[u8]) -> Result<Receipt> {
    if bytes.len() > 8192 {
        return Err(ProtocolError::Unsupported);
    }
    let value =
        crate::inspection::parse::parse_json(bytes).map_err(|_| ProtocolError::Unsupported)?;
    let r: Receipt = serde_json::from_value(value).map_err(|_| ProtocolError::Unsupported)?;
    validate_receipt(&r)?;
    Ok(r)
}
fn bound_item(item: &Item, depth: usize, count: &mut usize) -> Result<()> {
    *count += 1;
    if *count > MAX_NODES || depth > MAX_DEPTH {
        return Err(ProtocolError::Unsupported);
    }
    match item {
        Item::Table(t) => {
            for (_, i) in t.iter() {
                bound_item(i, depth + 1, count)?;
            }
        }
        Item::ArrayOfTables(a) => {
            for t in a.iter() {
                *count += 1;
                if *count > MAX_NODES || depth + 1 > MAX_DEPTH {
                    return Err(ProtocolError::Unsupported);
                }
                for (_, i) in t.iter() {
                    bound_item(i, depth + 2, count)?;
                }
            }
        }
        Item::Value(v) => bound_value(v, depth + 1, count)?,
        Item::None => {}
    }
    Ok(())
}
fn bound_value(value: &Value, depth: usize, count: &mut usize) -> Result<()> {
    *count += 1;
    if *count > MAX_NODES || depth > MAX_DEPTH {
        return Err(ProtocolError::Unsupported);
    }
    match value {
        Value::Array(a) => {
            for v in a.iter() {
                bound_value(v, depth + 1, count)?;
            }
        }
        Value::InlineTable(t) => {
            for (_, v) in t.iter() {
                bound_value(v, depth + 1, count)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn parsed(bytes: &[u8]) -> Result<(Document, String, bool)> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(ProtocolError::Unsupported);
    }
    let raw = std::str::from_utf8(bytes).map_err(|_| ProtocolError::Unsupported)?;
    let normalized = raw.replace("\r\n", "\n");
    let doc: Document = normalized.parse().map_err(|_| ProtocolError::Unsupported)?;
    bound_item(doc.as_item(), 0, &mut 0)?;
    // Unrecognized serialization changes are not silently accepted.
    if doc.to_string() != normalized {
        return Err(ProtocolError::Unsupported);
    }
    let table = doc
        .get("tui")
        .and_then(Item::as_table)
        .ok_or(ProtocolError::Unsupported)?;
    if table.is_implicit() || table.is_dotted() {
        return Err(ProtocolError::Unsupported);
    }
    let before = table
        .get("animations")
        .and_then(Item::as_bool)
        .ok_or(ProtocolError::Unsupported)?;
    Ok((doc, normalized, before))
}
fn boolean(v: bool) -> &'static [u8] {
    if v { b"true" } else { b"false" }
}
/// AST selects the field. Comparing serialized images identifies its token,
/// not text search; a checked CRLF map preserves every original non-token byte.
fn edit(bytes: &[u8], after: bool) -> Result<(bool, Vec<u8>)> {
    let (mut doc, normalized, before) = parsed(bytes)?;
    if before == after {
        return Ok((before, bytes.to_vec()));
    }
    let item = doc
        .as_table_mut()
        .get_mut("tui")
        .and_then(Item::as_table_mut)
        .and_then(|t| t.get_mut("animations"))
        .and_then(Item::as_value_mut)
        .ok_or(ProtocolError::Unsupported)?;
    let decor = item.decor().clone();
    *item = Value::from(after);
    *item.decor_mut() = decor;
    let changed = doc.to_string();
    let offset = normalized
        .bytes()
        .zip(changed.bytes())
        .position(|(a, b)| a != b)
        .ok_or(ProtocolError::Unsupported)?;
    let old = boolean(before);
    let new = boolean(after);
    if normalized.as_bytes().get(offset..offset + old.len()) != Some(old)
        || changed.as_bytes().get(offset..offset + new.len()) != Some(new)
        || normalized.as_bytes().get(offset + old.len()..)
            != changed.as_bytes().get(offset + new.len()..)
    {
        return Err(ProtocolError::Unsupported);
    }
    let (mut raw_offset, mut normalized_offset) = (0, 0);
    while normalized_offset < offset {
        if bytes.get(raw_offset..raw_offset + 2) == Some(b"\r\n") {
            raw_offset += 2;
        } else {
            raw_offset += 1;
        }
        normalized_offset += 1;
    }
    if bytes.get(raw_offset..raw_offset + old.len()) != Some(old) {
        return Err(ProtocolError::Unsupported);
    }
    let mut output = Vec::with_capacity(bytes.len() + 1);
    output.extend_from_slice(&bytes[..raw_offset]);
    output.extend_from_slice(new);
    output.extend_from_slice(&bytes[raw_offset + old.len()..]);
    let (_, _, actual) = parsed(&output)?;
    if actual != after {
        return Err(ProtocolError::Unsupported);
    }
    Ok((before, output))
}
fn token(bytes: &[u8], binding: &[u8], after: bool) -> Result<String> {
    if bytes.len() > MAX_BYTES {
        return Err(ProtocolError::Unsupported);
    }
    let mut h = Sha256::new();
    h.update(b"agentdeck/codex-animations/apply/1\0");
    h.update(SCHEMA_COMMIT);
    h.update(binding_digest(binding)?);
    h.update(digest(bytes));
    h.update([u8::from(after)]);
    Ok(format!("{:x}", h.finalize()))
}
pub fn preview(bytes: &[u8], binding: &[u8], after: bool) -> Result<Preview> {
    let (before, _) = edit(bytes, after)?;
    Ok(Preview {
        field: FIELD,
        before,
        after,
        changes: before != after,
        token: token(bytes, binding, after)?,
    })
}
pub fn prepare(
    bytes: &[u8],
    binding: &[u8],
    after: bool,
    expected_token: &str,
) -> Result<Option<PreparedChange>> {
    if token(bytes, binding, after)? != expected_token {
        return Err(ProtocolError::Changed);
    }
    let (before, output) = edit(bytes, after)?;
    if before == after {
        return Ok(None);
    }
    // The inverse must reproduce every byte, not merely equivalent TOML.
    if edit(&output, before)?.1 != bytes {
        return Err(ProtocolError::Unsupported);
    }
    let receipt = Receipt {
        format: 1,
        schema_commit: SCHEMA_COMMIT.into(),
        field: FIELD.into(),
        phase: Phase::Prepared,
        binding_digest: binding_digest(binding)?,
        before,
        after,
        before_digest: digest(bytes),
        after_digest: digest(&output),
        restore_digest: None,
    };
    Ok(Some(PreparedChange { output, receipt }))
}
fn validate_image(bytes: &[u8], binding: &[u8], r: &Receipt) -> Result<bool> {
    validate_receipt(r)?;
    if r.binding_digest != binding_digest(binding)? {
        return Err(ProtocolError::Changed);
    }
    if bytes.len() > MAX_BYTES {
        return Err(ProtocolError::Unsupported);
    }
    let hash = digest(bytes);
    if hash != r.before_digest && hash != r.after_digest {
        return Err(ProtocolError::Conflict);
    }
    let (_, _, value) = parsed(bytes)?;
    if hash == r.before_digest
        && value == r.before
        && digest(&edit(bytes, r.after)?.1) == r.after_digest
    {
        return Ok(false);
    }
    if hash == r.after_digest
        && value == r.after
        && digest(&edit(bytes, r.before)?.1) == r.before_digest
    {
        return Ok(true);
    }
    Err(ProtocolError::Conflict)
}
pub fn mark_applied(bytes: &[u8], binding: &[u8], receipt: &Receipt) -> Result<Receipt> {
    if receipt.phase != Phase::Prepared {
        return Err(ProtocolError::Unsupported);
    }
    if !validate_image(bytes, binding, receipt)? {
        return Err(ProtocolError::Conflict);
    }
    let mut r = receipt.clone();
    r.phase = Phase::Applied;
    Ok(r)
}
pub fn recovery(bytes: &[u8], binding: &[u8], receipt: &Receipt) -> Result<RecoveryAction> {
    let is_after = validate_image(bytes, binding, receipt)?;
    match (&receipt.phase, is_after) {
        (_, true) => Ok(RecoveryAction::RestoreOriginal),
        (Phase::Prepared | Phase::Restoring, false) => Ok(RecoveryAction::CleanupReceipt),
        (Phase::Applied, false) => Err(ProtocolError::Conflict),
    }
}
pub fn prepare_restore(
    bytes: &[u8],
    binding: &[u8],
    receipt: &Receipt,
) -> Result<Option<PreparedChange>> {
    if recovery(bytes, binding, receipt)? == RecoveryAction::CleanupReceipt {
        return Ok(None);
    }
    let output = edit(bytes, receipt.before)?.1;
    if digest(&output) != receipt.before_digest {
        return Err(ProtocolError::Conflict);
    }
    let mut r = receipt.clone();
    r.phase = Phase::Restoring;
    r.restore_digest = Some(r.before_digest.clone());
    Ok(Some(PreparedChange { output, receipt: r }))
}
#[cfg(test)]
mod tests;

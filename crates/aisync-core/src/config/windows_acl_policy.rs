//! Pure bounded policy for the ordinary Windows DACL subset we can preserve.
const SE_DACL_UNTRUSTED: u16 = 0x0040;
const SE_SERVER_SECURITY: u16 = 0x0080;

pub(super) fn validate(control: u16, acl: &[u8]) -> crate::Result<()> {
    let refused = || {
        crate::Error::InjectFailed(
        "Windows configuration DACL is outside the supported ordinary allow/deny subset; conditional, callback, object and server/untrusted security are refused".into())
    };
    if control & (SE_DACL_UNTRUSTED | SE_SERVER_SECURITY) != 0 || acl.len() < 8 {
        return Err(refused());
    }
    let word = |offset: usize| u16::from_le_bytes([acl[offset], acl[offset + 1]]) as usize;
    if !matches!(acl[0], 2 | 4) || word(2) != acl.len() {
        return Err(refused());
    }
    let mut offset = 8usize;
    for _ in 0..word(4) {
        let header = acl.get(offset..offset + 4).ok_or_else(refused)?;
        let length = u16::from_le_bytes([header[2], header[3]]) as usize;
        let ace = acl.get(offset..offset + length).ok_or_else(refused)?;
        // Ordinary ACE = header(4), access mask(4), SID(8 + 4*n).
        // No callback application data, object GUIDs or unknown trailing payload.
        if !matches!(header[0], 0 | 1) || header[1] & !0x1f != 0 || length < 16 {
            return Err(refused());
        }
        if ace[8] != 1 || ace[9] > 15 || length != 16 + 4 * ace[9] as usize {
            return Err(refused());
        }
        offset += length;
    }
    // ACL unused capacity after the declared ACEs is permitted by Windows.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary(kind: u8) -> Vec<u8> {
        vec![
            2, 0, 28, 0, 1, 0, 0, 0, kind, 0x10, 20, 0, 0xff, 1, 0x1f, 0, 1, 1, 0, 0, 0, 0, 0, 5,
            18, 0, 0, 0,
        ]
    }

    #[test]
    fn only_ordinary_allow_and_deny_ace_types_are_supported() {
        for kind in 0..=255 {
            assert_eq!(
                validate(0x9404, &ordinary(kind)).is_ok(),
                kind <= 1,
                "ACE type {kind}"
            );
        }
    }

    #[test]
    fn server_or_untrusted_control_bits_are_refused() {
        for control in [0x40, 0x80, 0xc0, 0x9444, 0x9484] {
            assert!(validate(control, &ordinary(0)).is_err());
        }
        assert!(validate(0x9404, &ordinary(0)).is_ok());
    }

    #[test]
    fn malformed_or_extended_ordinary_aces_are_refused() {
        let original = ordinary(0);
        for length in 0..original.len() {
            assert!(validate(0, &original[..length]).is_err());
        }
        for (offset, value) in [
            (0, 0),
            (4, 2),
            (9, 0x40),
            (10, 0),
            (10, 24),
            (16, 0),
            (17, 16),
            (17, 0),
        ] {
            let mut bad = original.clone();
            bad[offset] = value;
            assert!(validate(0, &bad).is_err(), "offset {offset}");
        }
        let empty = [2, 0, 8, 0, 0, 0, 0, 0];
        assert!(validate(0, &empty).is_ok());
    }
}

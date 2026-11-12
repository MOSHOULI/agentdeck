//! Preserve a bounded Windows DACL before staging any configuration bytes.
//! Owner/group changes are refused, not repaired. SACLs and streams are not copied.
use std::{
    fs::{File, OpenOptions},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_WRITE, HANDLE},
    Security::{
        Authorization::{SE_FILE_OBJECT, SetSecurityInfo},
        DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, GetKernelObjectSecurity,
        GetLengthSid, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
        GetSecurityDescriptorGroup, GetSecurityDescriptorOwner, IsValidAcl,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_AUTO_INHERIT_REQ,
        SE_DACL_AUTO_INHERITED, SE_DACL_PROTECTED, SetKernelObjectSecurity,
        UNPROTECTED_DACL_SECURITY_INFORMATION,
    },
    Storage::FileSystem::{FILE_FLAG_OPEN_REPARSE_POINT, READ_CONTROL, WRITE_DAC},
};

fn failure(message: &str) -> crate::Error {
    crate::Error::InjectFailed(format!(
        "Windows configuration security cannot be preserved: {message}; no permissions were automatically weakened"
    ))
}

#[derive(Clone, Debug)]
pub(super) struct Security {
    descriptor: Vec<u32>,
    acl: Vec<u8>,
    control: u16,
    owner: Vec<u8>,
    group: Vec<u8>,
}

impl PartialEq for Security {
    fn eq(&self, other: &Self) -> bool {
        self.acl == other.acl
            && self.control == other.control
            && self.owner == other.owner
            && self.group == other.group
    }
}
impl Eq for Security {}

impl Security {
    pub(super) fn read_path(path: &Path) -> crate::Result<Self> {
        let file = OpenOptions::new()
            .access_mode(READ_CONTROL)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        Self::read(&file)
    }

    pub(super) fn read(file: &File) -> crate::Result<Self> {
        let handle = file.as_raw_handle() as HANDLE;
        let information =
            DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION;
        let mut needed = 0;
        // SAFETY: borrowed live handle and writable size; first call only queries size.
        unsafe {
            GetKernelObjectSecurity(handle, information, std::ptr::null_mut(), 0, &mut needed);
        }
        if needed == 0 || needed > 1024 * 1024 {
            return Err(failure("security descriptor inspection failed"));
        }
        let mut descriptor = vec![0u32; (needed as usize).div_ceil(4)];
        // SAFETY: aligned, sufficiently sized owned buffer.
        if unsafe {
            GetKernelObjectSecurity(
                handle,
                information,
                descriptor.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err(failure("security descriptor inspection failed"));
        }
        let pointer = descriptor.as_mut_ptr().cast();
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = std::ptr::null_mut();
        // SAFETY: OS-validated descriptor; output pointers remain within live buffer.
        if unsafe { GetSecurityDescriptorDacl(pointer, &mut present, &mut acl, &mut defaulted) }
            == 0
            || present == 0
            || acl.is_null()
            || defaulted != 0
        {
            return Err(failure("absent, null or defaulted DACL is unsupported"));
        }
        if unsafe { IsValidAcl(acl) } == 0 {
            return Err(failure("invalid DACL"));
        }
        let mut control = 0;
        let mut revision = 0;
        if unsafe { GetSecurityDescriptorControl(pointer, &mut control, &mut revision) } == 0 {
            return Err(failure("DACL protection inspection failed"));
        }
        let mut owner = std::ptr::null_mut();
        let mut group = std::ptr::null_mut();
        if unsafe { GetSecurityDescriptorOwner(pointer, &mut owner, &mut defaulted) } == 0
            || owner.is_null()
            || unsafe { GetSecurityDescriptorGroup(pointer, &mut group, &mut defaulted) } == 0
            || group.is_null()
        {
            return Err(failure("owner/group inspection failed"));
        }
        // SAFETY: ACL and SID pointers are supplied by Windows for this descriptor.
        let acl = unsafe {
            std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec()
        };
        super::windows_acl_policy::validate(control, &acl)?;
        let owner = unsafe {
            std::slice::from_raw_parts(owner.cast::<u8>(), GetLengthSid(owner) as usize).to_vec()
        };
        let group = unsafe {
            std::slice::from_raw_parts(group.cast::<u8>(), GetLengthSid(group) as usize).to_vec()
        };
        Ok(Self {
            descriptor,
            acl,
            control: control
                & (SE_DACL_PROTECTED | SE_DACL_AUTO_INHERITED | SE_DACL_AUTO_INHERIT_REQ),
            owner,
            group,
        })
    }

    pub(super) fn apply_before_content(&self, file: &File) -> crate::Result<()> {
        let initial = Self::read(file)?;
        if self.owner != initial.owner || self.group != initial.group {
            return Err(failure("staging owner/group differs from original"));
        }
        let protection = if self.control & SE_DACL_PROTECTED != 0 {
            PROTECTED_DACL_SECURITY_INFORMATION
        } else {
            UNPROTECTED_DACL_SECURITY_INFORMATION
        };
        let mut descriptor = self.descriptor.clone();
        if self.control & SE_DACL_AUTO_INHERITED != 0 {
            // Preserve the current file inheritance model, including protected+AI.
            // SetKernelObjectSecurity loses AI on protected filesystem descriptors.
            let mut present = 0;
            let mut defaulted = 0;
            let mut dacl = std::ptr::null_mut();
            if unsafe {
                GetSecurityDescriptorDacl(
                    descriptor.as_mut_ptr().cast(),
                    &mut present,
                    &mut dacl,
                    &mut defaulted,
                )
            } == 0
                || present == 0
                || dacl.is_null()
                || defaulted != 0
            {
                return Err(failure("staging DACL extraction failed"));
            }
            // SAFETY: owned empty file handle; DACL belongs to the live aligned
            // descriptor. Owner, group and SACL are deliberately not modified.
            if unsafe {
                SetSecurityInfo(
                    file.as_raw_handle() as HANDLE,
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | protection,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    dacl,
                    std::ptr::null(),
                )
            } != 0
            {
                return Err(failure("staging DACL application failed"));
            }
        } else {
            // Bounded compatibility for legacy non-AI descriptors. The modern
            // file API converts the inheritance model; that is not preservation.
            // This legacy route is confined to our empty staging handle and
            // still requires exact ACL/control/owner/group verification below.
            if unsafe {
                SetKernelObjectSecurity(
                    file.as_raw_handle() as HANDLE,
                    DACL_SECURITY_INFORMATION | protection,
                    descriptor.as_mut_ptr().cast(),
                )
            } == 0
            {
                return Err(failure("staging DACL application failed"));
            }
        }
        let observed = Self::read(file)?;
        if observed != *self {
            // Fixed category words only: never expose descriptor bytes or SIDs.
            let reason = super::windows_acl_policy::verification_mismatch_reason(
                observed.acl != self.acl,
                observed.control != self.control,
                observed.owner != self.owner || observed.group != self.group,
            );
            return Err(failure(reason));
        }
        Ok(())
    }

    pub(super) fn verify_file(&self, file: &File) -> crate::Result<()> {
        if Self::read(file)? != *self {
            return Err(failure("configuration handle security changed"));
        }
        Ok(())
    }

    pub(super) fn verify_path(&self, path: &Path) -> crate::Result<()> {
        if Self::read_path(path)? != *self {
            return Err(failure("configuration security changed since snapshot"));
        }
        Ok(())
    }
}

pub(super) fn create(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_WRITE | READ_CONTROL | WRITE_DAC)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_owner_or_group_snapshot_refuses_before_staging_bytes() {
        let root =
            std::env::temp_dir().join(format!("agentdeck-owner-guard-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("empty");
        let file = create(&path).unwrap();
        let original = Security::read(&file).unwrap();
        for owner in [true, false] {
            let mut changed_snapshot = original.clone();
            if owner {
                changed_snapshot.owner.push(0);
            } else {
                changed_snapshot.group.push(0);
            }
            assert!(changed_snapshot.apply_before_content(&file).is_err());
            assert_eq!(file.metadata().unwrap().len(), 0);
            assert_eq!(Security::read(&file).unwrap(), original);
        }
        drop(file);
        std::fs::remove_dir_all(root).unwrap();
    }
}

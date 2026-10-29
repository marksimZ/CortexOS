use super::Record;
use crate::models::AppResult;
use zeroize::Zeroizing;

pub trait Vault: Send + Sync {
    fn read(&self) -> AppResult<Option<Record>>;
    fn write(&self, record: &Record) -> AppResult<()>;
    fn delete(&self) -> AppResult<()>;
}

pub struct WindowsVault {
    target: String,
}
impl WindowsVault {
    #[cfg(test)]
    pub fn test_target(target: String) -> Self {
        Self { target }
    }
    pub fn new() -> Self {
        // Debug and release never share credentials. Tests additionally get a unique scope.
        let mut target = "CortexOS/PersonalizedBenchmark/v1".to_string();
        #[cfg(debug_assertions)]
        {
            target.push_str("/debug");
            if let Ok(directory) = std::env::var("CORTEXOS_TEST_DATA_DIR") {
                use sha2::{Digest, Sha256};
                target.push_str(&format!("/{:x}", Sha256::digest(directory.as_bytes())));
            }
        }
        Self { target }
    }
}

#[cfg(windows)]
impl Vault for WindowsVault {
    fn read(&self) -> AppResult<Option<Record>> {
        use windows_sys::Win32::{Foundation::*, Security::Credentials::*};
        let target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        let mut pointer = std::ptr::null_mut();
        // Windows owns the returned buffer until CredFree; wipe its blob before release.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) == 0 {
                return if GetLastError() == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(vault_error())
                };
            }
            let credential = &*pointer;
            let result = if credential.CredentialBlobSize == 0
                || credential.CredentialBlobSize > 2560
                || credential.CredentialBlob.is_null()
            {
                Err(vault_error())
            } else {
                let blob = std::slice::from_raw_parts_mut(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                );
                let result = serde_json::from_slice::<Record>(blob).map_err(|_| vault_error());
                use zeroize::Zeroize;
                blob.zeroize();
                result
            };
            CredFree(pointer.cast());
            let record = result?;
            record.validate()?;
            Ok(Some(record))
        }
    }
    fn write(&self, record: &Record) -> AppResult<()> {
        use windows_sys::Win32::Security::Credentials::*;
        record.validate()?;
        let mut blob = Zeroizing::new(serde_json::to_vec(record).map_err(|_| vault_error())?);
        if blob.len() > 2560 {
            return Err(vault_error());
        }
        let mut target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        let mut user: Vec<u16> = "CortexOS\0".encode_utf16().collect();
        let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target.as_mut_ptr();
        credential.UserName = user.as_mut_ptr();
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
        credential.CredentialBlob = blob.as_mut_ptr();
        credential.CredentialBlobSize = blob.len() as u32;
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            Err(vault_error())
        } else {
            Ok(())
        }
    }
    fn delete(&self) -> AppResult<()> {
        use windows_sys::Win32::{Foundation::*, Security::Credentials::*};
        let target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0
            || unsafe { GetLastError() } == ERROR_NOT_FOUND
        {
            Ok(())
        } else {
            Err(vault_error())
        }
    }
}
#[cfg(not(windows))]
impl Vault for WindowsVault {
    fn read(&self) -> AppResult<Option<Record>> {
        Err(vault_error())
    }
    fn write(&self, _: &Record) -> AppResult<()> {
        Err(vault_error())
    }
    fn delete(&self) -> AppResult<()> {
        Err(vault_error())
    }
}
fn vault_error() -> String {
    "系统凭据存储不可用，请检查 Windows 登录状态后重试。密钥未写入普通文件。".into()
}

//! Secrets never implement Debug/Serialize or cross the backend boundary after entry.
use crate::vision_provider::VisionError;
use zeroize::Zeroizing;
pub const CREDENTIAL_TARGET: &str = "ScreenshotRenamer/OpenAI";
pub struct Secret(Zeroizing<String>);
impl Secret {
    pub fn new(value: String) -> Result<Self, VisionError> {
        let value = Zeroizing::new(value);
        if value.is_empty()
            || value.len() > 2048
            || value
                .bytes()
                .any(|c| c.is_ascii_control() || c.is_ascii_whitespace())
        {
            return Err(VisionError::Configuration);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
pub trait CredentialStore: Send + Sync {
    fn exists(&self) -> Result<bool, VisionError> {
        self.load().map(|_| true)
    }
    fn load(&self) -> Result<Secret, VisionError>;
    fn save(&self, secret: Secret) -> Result<(), VisionError>;
    fn remove(&self) -> Result<(), VisionError>;
}
pub struct OsCredentials {
    target: String,
}
impl Default for OsCredentials {
    fn default() -> Self {
        Self::new(CREDENTIAL_TARGET)
    }
}
impl OsCredentials {
    pub fn new(target: &str) -> Self {
        Self {
            target: target.into(),
        }
    }
}
#[cfg(windows)]
impl CredentialStore for OsCredentials {
    fn exists(&self) -> Result<bool, VisionError> {
        use windows::{
            core::PCWSTR,
            Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
        };
        let target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        let mut ptr = std::ptr::null_mut();
        match unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut ptr) } {
            Ok(()) => {
                unsafe {
                    let c = &*ptr;
                    if c.CredentialBlobSize > 0 && !c.CredentialBlob.is_null() {
                        use zeroize::Zeroize;
                        std::slice::from_raw_parts_mut(
                            c.CredentialBlob,
                            c.CredentialBlobSize as usize,
                        )
                        .zeroize();
                    }
                    CredFree(ptr.cast());
                }
                Ok(true)
            }
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(false),
            Err(_) => Err(VisionError::Configuration),
        }
    }
    fn load(&self) -> Result<Secret, VisionError> {
        use windows::{core::PCWSTR, Win32::Security::Credentials::*};
        let target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        let mut ptr = std::ptr::null_mut();
        unsafe {
            CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut ptr)
                .map_err(|_| VisionError::Configuration)?;
            let credential = &*ptr;
            let result = if credential.CredentialBlobSize == 0
                || credential.CredentialBlobSize > 2048
                || credential.CredentialBlob.is_null()
            {
                Err(VisionError::Configuration)
            } else {
                let bytes = std::slice::from_raw_parts_mut(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                );
                let result = std::str::from_utf8(bytes)
                    .map_err(|_| VisionError::Configuration)
                    .and_then(|s| Secret::new(s.to_owned()));
                use zeroize::Zeroize;
                bytes.zeroize();
                result
            };
            CredFree(ptr.cast());
            result
        }
    }
    fn save(&self, secret: Secret) -> Result<(), VisionError> {
        use windows::{core::PWSTR, Win32::Security::Credentials::*};
        let mut target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        let mut user: Vec<u16> = "OpenAI".encode_utf16().chain(Some(0)).collect();
        let mut bytes = Zeroizing::new(secret.expose().as_bytes().to_vec());
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            UserName: PWSTR(user.as_mut_ptr()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0).map_err(|_| VisionError::Configuration) }
    }
    fn remove(&self) -> Result<(), VisionError> {
        use windows::{
            core::PCWSTR,
            Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
        };
        let target: Vec<u16> = self.target.encode_utf16().chain(Some(0)).collect();
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(_) => Err(VisionError::Configuration),
        }
    }
}
#[cfg(not(windows))]
impl CredentialStore for OsCredentials {
    fn load(&self) -> Result<Secret, VisionError> {
        Err(VisionError::Configuration)
    }
    fn save(&self, _: Secret) -> Result<(), VisionError> {
        Err(VisionError::Configuration)
    }
    fn remove(&self) -> Result<(), VisionError> {
        Err(VisionError::Configuration)
    }
}

//! Saved passwords, sealed with Windows' DPAPI: only the same Windows user
//! on the same machine can open them, so a copied `login.ini` gives
//! nothing away. Elsewhere passwords are not saved.

/// Seals `secret` for this Windows user, as hexadecimal for an ini line.
pub fn seal(secret: &str) -> Option<String> {
    let sealed = imp::protect(secret.as_bytes())?;
    Some(sealed.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Opens what [`seal`] made; `None` for another user's or a damaged seal.
pub fn open(sealed: &str) -> Option<String> {
    if sealed.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..sealed.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(sealed.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(imp::unprotect(&bytes)?).ok()
}

/// Whether passwords can be saved here at all.
pub const AVAILABLE: bool = cfg!(windows);

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };

    /// Mixed into every seal, so other programs' DPAPI blobs do not open
    /// as ours.
    const ENTROPY: &[u8] = b"gravak-login-v1";

    fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr().cast_mut(),
        }
    }

    /// Copies DPAPI's output and frees it.
    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        // SAFETY: DPAPI filled `out` with `cbData` bytes it allocated with
        // LocalAlloc; they are copied before being freed.
        unsafe {
            let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            LocalFree(out.pbData.cast());
            bytes
        }
    }

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        let input = blob(plain);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: every pointer is to a live blob for the call's duration.
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        // SAFETY: on success DPAPI allocated `out`.
        (ok != 0).then(|| unsafe { take(out) })
    }

    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        let input = blob(sealed);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: as in `protect`.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        // SAFETY: on success DPAPI allocated `out`.
        (ok != 0).then(|| unsafe { take(out) })
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn protect(_plain: &[u8]) -> Option<Vec<u8>> {
        None
    }

    pub fn unprotect(_sealed: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn a_sealed_password_opens_again() {
        let sealed = super::seal("s3cret!").expect("DPAPI seals");
        assert!(!sealed.contains("s3cret"));
        assert_eq!(super::open(&sealed).as_deref(), Some("s3cret!"));
        assert_eq!(super::open("00ff"), None);
        assert_eq!(super::open("xyz"), None);
    }
}

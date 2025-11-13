use crate::ffi;

pub fn decrypt(ciphertext: &mut [u8], output: &mut [u8]) -> Result<(), i32> {
    let result = unsafe {
        ffi::csp_crypto_decrypt(
            ciphertext.as_mut_ptr(),
            ciphertext.len() as u8,
            output.as_mut_ptr(),
        )
    };
    if result < 0 {
        Err(result)
    } else {
        Ok(())
    }
}

pub fn encrypt(plaintext: &mut [u8], output: &mut [u8]) -> Result<usize, i32> {
    let result = unsafe {
        ffi::csp_crypto_encrypt(plaintext.as_mut_ptr(), plaintext.len() as u8, output.as_mut_ptr())
    };
    if result < 0 {
        Err(result)
    } else {
        Ok(result as usize)
    }
}

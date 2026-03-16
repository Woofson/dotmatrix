//! Encryption and decryption utilities using age crate.
//!
//! This module provides password-based encryption using the age-encryption.org standard.
//! Files are encrypted with a user-provided password before being stored in the backup.

use age::secrecy::SecretString;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::Path;

/// Encrypt file contents using password-based encryption.
///
/// Reads the source file, encrypts it with the provided password using age,
/// and writes the encrypted content to the destination.
pub fn encrypt_file(source: &Path, dest: &Path, password: &SecretString) -> Result<()> {
    let encryptor = age::Encryptor::with_user_passphrase(password.clone());

    let input = std::fs::read(source)
        .with_context(|| format!("Failed to read source file: {}", source.display()))?;

    let mut output = vec![];
    let mut writer = encryptor
        .wrap_output(&mut output)
        .context("Failed to create age encryptor")?;

    writer.write_all(&input)
        .context("Failed to write encrypted data")?;

    writer.finish()
        .context("Failed to finalize encryption")?;

    // Create parent directories if needed
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(dest, output)
        .with_context(|| format!("Failed to write encrypted file: {}", dest.display()))?;

    Ok(())
}

/// Decrypt file contents using password-based encryption.
///
/// Reads the encrypted source file, decrypts it with the provided password,
/// and writes the decrypted content to the destination.
pub fn decrypt_file(source: &Path, dest: &Path, password: &SecretString) -> Result<()> {
    let encrypted = std::fs::read(source)
        .with_context(|| format!("Failed to read encrypted file: {}", source.display()))?;

    let decryptor = age::Decryptor::new(&encrypted[..])
        .context("Failed to parse encrypted file")?;

    let identity = age::scrypt::Identity::new(password.clone());

    let mut decrypted = vec![];
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| anyhow::anyhow!("Decryption failed (wrong password?): {}", e))?;

    reader.read_to_end(&mut decrypted)
        .context("Failed to read decrypted data")?;

    // Create parent directories if needed
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(dest, decrypted)
        .with_context(|| format!("Failed to write decrypted file: {}", dest.display()))?;

    Ok(())
}

/// Encrypt data in memory and return encrypted bytes.
///
/// Useful for encrypting file contents before hashing or storing.
pub fn encrypt_bytes(data: &[u8], password: &SecretString) -> Result<Vec<u8>> {
    let encryptor = age::Encryptor::with_user_passphrase(password.clone());

    let mut output = vec![];
    let mut writer = encryptor
        .wrap_output(&mut output)
        .context("Failed to create age encryptor")?;

    writer.write_all(data)
        .context("Failed to write encrypted data")?;

    writer.finish()
        .context("Failed to finalize encryption")?;

    Ok(output)
}

/// Decrypt data in memory and return decrypted bytes.
pub fn decrypt_bytes(encrypted: &[u8], password: &SecretString) -> Result<Vec<u8>> {
    let decryptor = age::Decryptor::new(encrypted)
        .context("Failed to parse encrypted data")?;

    let identity = age::scrypt::Identity::new(password.clone());

    let mut decrypted = vec![];
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| anyhow::anyhow!("Decryption failed (wrong password?): {}", e))?;

    reader.read_to_end(&mut decrypted)
        .context("Failed to read decrypted data")?;

    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn test_password() -> SecretString {
        SecretString::from("test-password-123")
    }

    #[test]
    fn test_encrypt_decrypt_bytes() {
        let original = b"Hello, world! This is a test message.";
        let password = test_password();

        let encrypted = encrypt_bytes(original, &password).unwrap();
        assert_ne!(&encrypted[..], original);

        let decrypted = decrypt_bytes(&encrypted, &password).unwrap();
        assert_eq!(&decrypted[..], original);
    }

    #[test]
    fn test_encrypt_decrypt_file() {
        let password = test_password();
        let original_content = b"Test file content for encryption";

        // Create source file
        let mut source = NamedTempFile::new().unwrap();
        source.write_all(original_content).unwrap();

        // Create destination paths
        let encrypted_file = NamedTempFile::new().unwrap();
        let decrypted_file = NamedTempFile::new().unwrap();

        // Encrypt
        encrypt_file(source.path(), encrypted_file.path(), &password).unwrap();

        // Verify encrypted file is different
        let encrypted_content = std::fs::read(encrypted_file.path()).unwrap();
        assert_ne!(&encrypted_content[..], original_content);

        // Decrypt
        decrypt_file(encrypted_file.path(), decrypted_file.path(), &password).unwrap();

        // Verify decrypted content matches original
        let decrypted_content = std::fs::read(decrypted_file.path()).unwrap();
        assert_eq!(&decrypted_content[..], original_content);
    }

    #[test]
    fn test_wrong_password_fails() {
        let original = b"Secret data";
        let correct_password = test_password();
        let wrong_password = SecretString::from("wrong-password");

        let encrypted = encrypt_bytes(original, &correct_password).unwrap();

        let result = decrypt_bytes(&encrypted, &wrong_password);
        assert!(result.is_err());
    }
}

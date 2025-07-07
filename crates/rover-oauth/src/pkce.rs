use crate::{error::OAuthError, types::PkceChallenge};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};

/// Generate PKCE parameters according to RFC 7636
pub fn generate_pkce_challenge() -> Result<PkceChallenge, OAuthError> {
    // Generate code_verifier: 43-128 characters, URL-safe
    let code_verifier = generate_code_verifier()?;
    
    // Generate code_challenge using S256 method
    let code_challenge = generate_code_challenge(&code_verifier)?;
    
    Ok(PkceChallenge {
        code_verifier,
        code_challenge,
        code_challenge_method: "S256".to_string(),
    })
}

/// Generate a cryptographically random code verifier
/// Length: 43-128 characters (we use 128 for maximum entropy)
/// Characters: [A-Z] / [a-z] / [0-9] / "-" / "." / "_" / "~"
fn generate_code_verifier() -> Result<String, OAuthError> {
    // Generate 32 bytes (256 bits) of random data
    let mut rng = rand::rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    
    // Base64url encode without padding (RFC 7636 compliant)
    let code_verifier = URL_SAFE_NO_PAD.encode(&random_bytes);
    
    if code_verifier.len() < 43 || code_verifier.len() > 128 {
        return Err(OAuthError::PkceError(
            "Code verifier length must be between 43 and 128 characters".to_string(),
        ));
    }
    
    Ok(code_verifier)
}

/// Generate code challenge using SHA256 and base64url encoding
fn generate_code_challenge(code_verifier: &str) -> Result<String, OAuthError> {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    
    // Base64url encode without padding
    let code_challenge = URL_SAFE_NO_PAD.encode(&hash);
    
    Ok(code_challenge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce_challenge() {
        let challenge = generate_pkce_challenge().unwrap();
        
        // Verify code_verifier length
        assert!(challenge.code_verifier.len() >= 43);
        assert!(challenge.code_verifier.len() <= 128);
        
        // Verify code_challenge is base64url encoded
        assert!(!challenge.code_challenge.is_empty());
        assert!(!challenge.code_challenge.contains('='));
        assert!(!challenge.code_challenge.contains('+'));
        assert!(!challenge.code_challenge.contains('/'));
        
        // Verify method
        assert_eq!(challenge.code_challenge_method, "S256");
    }

    #[test]
    fn test_code_challenge_consistency() {
        let code_verifier = "test_verifier_with_sufficient_length_for_pkce_requirements_12345678901234567890";
        let challenge1 = generate_code_challenge(code_verifier).unwrap();
        let challenge2 = generate_code_challenge(code_verifier).unwrap();
        
        // Same input should produce same output
        assert_eq!(challenge1, challenge2);
    }

    #[test]
    fn test_different_verifiers_produce_different_challenges() {
        let challenge1 = generate_pkce_challenge().unwrap();
        let challenge2 = generate_pkce_challenge().unwrap();
        
        // Different verifiers should produce different challenges
        assert_ne!(challenge1.code_verifier, challenge2.code_verifier);
        assert_ne!(challenge1.code_challenge, challenge2.code_challenge);
    }
}
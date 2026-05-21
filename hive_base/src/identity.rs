use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use rand::rngs::OsRng;
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentIdentity {
    pub id: Uuid,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl AgentIdentity {
    pub fn new() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            id: Uuid::new_v4(),
            signing_key,
            verifying_key,
        }
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn sign_data(&self, data: &[u8]) -> [u8; 64] {
        self.signing_key.sign(data).to_bytes()
    }

    pub fn verify(&self, data: &[u8], signature_bytes: &[u8; 64]) -> bool {
        let sig = Signature::from_bytes(signature_bytes);
        self.verifying_key.verify_strict(data, &sig).is_ok()
    }

    pub fn verify_with_key(
        verifying_key_bytes: &[u8; 32],
        data: &[u8],
        signature_bytes: &[u8; 64],
    ) -> bool {
        let vk = match VerifyingKey::from_bytes(verifying_key_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(signature_bytes);
        vk.verify_strict(data, &sig).is_ok()
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
}

impl Default for AgentIdentity {
    fn default() -> Self {
        Self::new()
    }
}

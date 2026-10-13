//! Public synthetic encryption fixtures over the repository's original forms corpus.
#![allow(clippy::unwrap_used)]
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions, dictionary};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn fixture(revision: u8, password: &str, signed: bool) -> (Vec<u8>, Vec<u8>) {
    fixture_from(
        include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
        revision,
        password,
        signed,
    )
}

pub fn fixture_from(
    input: &[u8],
    revision: u8,
    password: &str,
    signed: bool,
) -> (Vec<u8>, Vec<u8>) {
    let mut document = Document::load_mem(input).unwrap();
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("native-encrypted-fixture"),
            Object::string_literal("native-encrypted-fixture"),
        ],
    );
    if signed {
        let marker = document.add_object(dictionary! {"Type"=>"Sig", "Contents"=>Object::string_literal("synthetic signature marker")});
        document.trailer.set("SyntheticSignatureEvidence", marker);
    }
    let mut plain = Vec::new();
    document.save_to(&mut plain).unwrap();
    let state = match revision {
        2 => EncryptionState::try_from(EncryptionVersion::V1 {
            document: &document,
            owner_password: "synthetic-owner",
            user_password: password,
            permissions: Permissions::all(),
        }),
        3 => EncryptionState::try_from(EncryptionVersion::V2 {
            document: &document,
            owner_password: "synthetic-owner",
            user_password: password,
            key_length: 128,
            permissions: Permissions::all(),
        }),
        4 => {
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
            EncryptionState::try_from(EncryptionVersion::V4 {
                document: &document,
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: "synthetic-owner",
                user_password: password,
                permissions: Permissions::all(),
            })
        }
        6 => {
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
            EncryptionState::try_from(EncryptionVersion::V5 {
                encrypt_metadata: true,
                file_encryption_key: &[0x31; 32],
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: "synthetic-owner",
                user_password: password,
                permissions: Permissions::all(),
            })
        }
        _ => panic!("unsupported test revision"),
    }
    .unwrap();
    assert_eq!(
        state.encode().unwrap().get(b"R").unwrap().as_i64().unwrap(),
        i64::from(revision)
    );
    document.encrypt(&state).unwrap();
    let mut cipher = Vec::new();
    document.save_to(&mut cipher).unwrap();
    (cipher, plain)
}

use crate::{
    derivation::{attached_signature_code::get_sig_count, self_addressing::SelfAddressing},
    error::Error,
    event::{
        event_data::{inception::InceptionEvent, EventData},
        Event,
    },
    prefix::{AttachedSignaturePrefix, BasicPrefix, IdentifierPrefix, Prefix, SelfSigningPrefix},
    state::{EventSemantics, IdentifierState},
    util::dfs_serializer,
};
pub mod serialization_info;
use serde::{Deserialize, Serialize};
use serialization_info::*;
pub mod parse;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventMessage {
    /// Serialization Information
    ///
    /// Encodes the version, size and serialization format of the event
    #[serde(rename = "vs")]
    pub serialization_info: SerializationInfo,

    #[serde(flatten)]
    pub event: Event,
    // Additional Data for forwards compat
    //
    // TODO: Currently seems to be bugged, it captures and duplicates every element in the event
    // #[serde(flatten)]
    // pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct SignedEventMessage {
    pub event_message: EventMessage,
    pub signatures: Vec<AttachedSignaturePrefix>,
}

#[derive(Debug, Clone)]
pub struct SignedNontransferableReceipt {
    pub body: EventMessage,
    pub couplets: Vec<(BasicPrefix, SelfSigningPrefix)>,
}

impl EventMessage {
    pub fn new(event: Event, format: SerializationFormats) -> Result<Self, Error> {
        Ok(Self {
            serialization_info: SerializationInfo::new(format, Self::get_size(&event, format)?),
            event,
        })
    }

    fn get_size(event: &Event, format: SerializationFormats) -> Result<usize, Error> {
        Ok(Self {
            serialization_info: SerializationInfo::new(format, 0),
            event: event.clone(),
        }
        .serialize()?
        .len())
    }

    pub fn serialization(&self) -> SerializationFormats {
        self.serialization_info.kind
    }

    /// Get Inception Data
    ///
    /// Strips prefix and version string length info from an event
    /// used for verifying identifier binding for self-addressing and self-certifying
    pub fn get_inception_data(
        icp: &InceptionEvent,
        code: SelfAddressing,
        format: SerializationFormats,
    ) -> Result<Vec<u8>, Error> {
        // use dummy prefix to get correct size info
        // TODO: dynamically size dummy derivative, non-32 byte prefixes will fail
        let icp_event_data = Event {
            prefix: IdentifierPrefix::SelfAddressing(code.derive(&[0u8; 32])),
            sn: 0,
            event_data: EventData::Icp(icp.clone()),
        };
        Ok(dfs_serializer::to_vec(&Self {
            serialization_info: icp_event_data
                .clone()
                .to_message(format)
                .unwrap()
                .serialization_info,
            event: Event {
                // default prefix serializes to empty string
                prefix: IdentifierPrefix::default(),
                ..icp_event_data
            },
        })?)
    }

    /// Serialize
    ///
    /// returns the serialized event message
    /// NOTE: this method, for deserialized events, will be UNABLE to preserve ordering
    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        self.serialization().encode(self)
    }

    pub fn sign(&self, sigs: Vec<AttachedSignaturePrefix>) -> SignedEventMessage {
        SignedEventMessage::new(self, sigs)
    }
}

impl SignedEventMessage {
    pub fn new(message: &EventMessage, sigs: Vec<AttachedSignaturePrefix>) -> Self {
        Self {
            event_message: message.clone(),
            signatures: sigs,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        Ok([
            self.event_message.serialize()?,
            get_sig_count(self.signatures.len() as u16)
                .as_bytes()
                .to_vec(),
            self.signatures
                .iter()
                .map(|sig| sig.to_str().as_bytes().to_vec())
                .fold(vec![], |acc, next| [acc, next].concat()),
        ]
        .concat())
    }
}

impl EventSemantics for EventMessage {
    fn apply_to(&self, state: IdentifierState) -> Result<IdentifierState, Error> {
        // Update state.last with serialized current event message.
        match self.event.event_data {
            EventData::Icp(_) => {
                if verify_identifier_binding(self)? {
                    self.event.apply_to(IdentifierState {
                        last: self.serialize()?,
                        ..state
                    })
                } else {
                    Err(Error::SemanticError(
                        "Invalid Identifier Prefix Binding".into(),
                    ))
                }
            }
            EventData::Rot(ref rot) => {
                // Event may be out of order or duplicated, so before checking
                // previous event hash binding and update state last, apply it
                // to the state. It will return EventOutOfOrderError or
                // EventDuplicateError in that cases.
                self.event.apply_to(state.clone()).and_then(|next_state| {
                    if rot.previous_event_hash.verify_binding(&state.last) {
                        Ok(IdentifierState {
                            last: self.serialize()?,
                            ..next_state
                        })
                    } else {
                        Err(Error::SemanticError(
                            "Last event does not match previous event".to_string(),
                        ))
                    }
                })
            }
            EventData::Ixn(ref inter) => {
                self.event.apply_to(state.clone()).and_then(|next_state| {
                    if inter.previous_event_hash.verify_binding(&state.last) {
                        Ok(IdentifierState {
                            last: self.serialize()?,
                            ..next_state
                        })
                    } else {
                        Err(Error::SemanticError(
                            "Last event does not match previous event".to_string(),
                        ))
                    }
                })
            }
            _ => self.event.apply_to(state),
        }
    }
}

impl EventSemantics for SignedEventMessage {
    fn apply_to(&self, state: IdentifierState) -> Result<IdentifierState, Error> {
        self.event_message.apply_to(state)
    }
}

pub fn verify_identifier_binding(icp_event: &EventMessage) -> Result<bool, Error> {
    match &icp_event.event.event_data {
        EventData::Icp(icp) => match &icp_event.event.prefix {
            IdentifierPrefix::Basic(bp) => Ok(icp.key_config.public_keys.len() == 1
                && bp == icp.key_config.public_keys.first().unwrap()),
            IdentifierPrefix::SelfAddressing(sap) => Ok(sap.verify_binding(
                &EventMessage::get_inception_data(&icp, sap.derivation, icp_event.serialization())?,
            )),
            IdentifierPrefix::SelfSigning(_ssp) => todo!(),
        },
        _ => Err(Error::SemanticError("Not an ICP event".into())),
    }
}

#[cfg(test)]
mod tests {
    mod test_utils;
    use self::test_utils::{test_mock_event_sequence, EventType};
    use super::*;
    use crate::{
        derivation::{basic::Basic, self_addressing::SelfAddressing, self_signing::SelfSigning},
        event::{
            event_data::{inception::InceptionEvent, interaction::InteractionEvent, EventData},
            sections::InceptionWitnessConfig,
            sections::KeyConfig,
        },
        prefix::{AttachedSignaturePrefix, IdentifierPrefix, SelfAddressingPrefix},
    };
    use serde_json;
    use ursa::{
        kex::{x25519, KeyExchangeScheme},
        signatures::{ed25519, SignatureScheme},
    };

    #[test]
    fn basic_create() -> Result<(), Error> {
        // hi Ed!
        let ed = ed25519::Ed25519Sha512::new();

        // get two ed25519 keypairs
        let (pub_key0, priv_key0) = ed
            .keypair(Option::None)
            .map_err(|e| Error::CryptoError(e))?;
        let (pub_key1, _priv_key1) = ed
            .keypair(Option::None)
            .map_err(|e| Error::CryptoError(e))?;

        // initial signing key prefix
        let pref0 = Basic::Ed25519.derive(pub_key0);

        // initial control key hash prefix
        let pref1 = Basic::Ed25519.derive(pub_key1);
        let nxt = SelfAddressing::Blake3_256.derive(pref1.to_str().as_bytes());

        // create a simple inception event
        let icp = Event {
            prefix: IdentifierPrefix::Basic(pref0.clone()),
            sn: 0,
            event_data: EventData::Icp(InceptionEvent {
                key_config: KeyConfig::new(vec![pref0.clone()], nxt.clone(), Some(1)),
                witness_config: InceptionWitnessConfig::default(),
                inception_configuration: vec![],
            }),
        };

        let icp_m = icp.to_message(SerializationFormats::JSON)?;

        // serialised message
        let ser = icp_m.serialize()?;

        // sign
        let sig = ed
            .sign(&ser, &priv_key0)
            .map_err(|e| Error::CryptoError(e))?;
        let attached_sig = AttachedSignaturePrefix::new(SelfSigning::Ed25519Sha512, sig, 0);

        assert!(pref0.verify(&ser, &attached_sig.signature)?);

        let signed_event = icp_m.sign(vec![attached_sig]);

        let s_ = IdentifierState::default();

        let s0 = s_.apply(&signed_event)?;

        assert!(s0.current.verify(&ser, &signed_event.signatures)?);

        assert_eq!(s0.prefix, IdentifierPrefix::Basic(pref0.clone()));
        assert_eq!(s0.sn, 0);
        assert_eq!(s0.last, ser);
        assert_eq!(s0.current.public_keys.len(), 1);
        assert_eq!(s0.current.public_keys[0], pref0);
        assert_eq!(s0.current.threshold, 1);
        assert_eq!(s0.current.threshold_key_digest, nxt);
        assert_eq!(s0.witnesses, vec![]);
        assert_eq!(s0.tally, 0);
        assert_eq!(s0.delegated_keys, vec![]);

        Ok(())
    }

    #[test]
    fn self_addressing_create() -> Result<(), Error> {
        // hi Ed!
        let ed = ed25519::Ed25519Sha512::new();

        let (sig_key_0, sig_priv_0) = ed
            .keypair(Option::None)
            .map_err(|e| Error::CryptoError(e))?;
        let (sig_key_1, sig_priv_1) = ed
            .keypair(Option::None)
            .map_err(|e| Error::CryptoError(e))?;

        // hi X!
        let x = x25519::X25519Sha256::new();

        // get two X25519 keypairs
        let (enc_key_0, enc_priv_0) = x.keypair(Option::None).map_err(|e| Error::CryptoError(e))?;
        let (enc_key_1, enc_priv_1) = x.keypair(Option::None).map_err(|e| Error::CryptoError(e))?;

        // initial key set
        let sig_pref_0 = Basic::Ed25519.derive(sig_key_0);
        let enc_pref_0 = Basic::X25519.derive(enc_key_0);

        // next key set
        let sig_pref_1 = Basic::Ed25519.derive(sig_key_1);
        let enc_pref_1 = Basic::X25519.derive(enc_key_1);

        // next key set pre-commitment
        let nexter_pref = SelfAddressing::Blake3_256.derive(
            [sig_pref_1.to_str(), enc_pref_1.to_str()]
                .join("")
                .as_bytes(),
        );

        let icp = InceptionEvent::new(
            KeyConfig::new(
                vec![sig_pref_0.clone(), enc_pref_0.clone()],
                nexter_pref.clone(),
                Some(1),
            ),
            None,
            None,
        )
        .incept_self_addressing(SelfAddressing::Blake3_256, SerializationFormats::JSON)?;

        // serialised
        let serialized = icp.serialize()?;

        // sign
        let sig = ed
            .sign(&serialized, &sig_priv_0)
            .map_err(|e| Error::CryptoError(e))?;
        let attached_sig = AttachedSignaturePrefix::new(SelfSigning::Ed25519Sha512, sig, 0);

        assert!(sig_pref_0.verify(&serialized, &attached_sig.signature)?);

        let signed_event = icp.sign(vec![attached_sig]);

        let s_ = IdentifierState::default();

        let s0 = s_.apply(&signed_event)?;

        assert!(s0.current.verify(&serialized, &signed_event.signatures)?);

        assert_eq!(s0.prefix, icp.event.prefix);
        assert_eq!(s0.sn, 0);
        assert_eq!(s0.last, serialized);
        assert_eq!(s0.current.public_keys.len(), 2);
        assert_eq!(s0.current.public_keys[0], sig_pref_0);
        assert_eq!(s0.current.public_keys[1], enc_pref_0);
        assert_eq!(s0.current.threshold, 1);
        assert_eq!(s0.current.threshold_key_digest, nexter_pref);
        assert_eq!(s0.witnesses, vec![]);
        assert_eq!(s0.tally, 0);
        assert_eq!(s0.delegated_keys, vec![]);

        Ok(())
    }

    #[test]
    fn test_basic_establishment_sequence() -> Result<(), Error> {
        // Sequence should contain Inception Event.
        let no_inception_seq = vec![EventType::Rotation, EventType::Rotation];
        assert!(test_mock_event_sequence(no_inception_seq).is_err());

        // Sequence can't start with Rotation Event.
        let rotation_first_seq = vec![EventType::Rotation, EventType::Inception];
        assert!(test_mock_event_sequence(rotation_first_seq).is_err());

        // Sequence should contain exacly one Inception Event.
        let wrong_seq = vec![
            EventType::Inception,
            EventType::Rotation,
            EventType::Rotation,
            EventType::Inception,
        ];
        assert!(test_mock_event_sequence(wrong_seq).is_err());

        let ok_seq = vec![
            EventType::Inception,
            EventType::Rotation,
            EventType::Rotation,
        ];
        assert!(test_mock_event_sequence(ok_seq).is_ok());

        Ok(())
    }

    #[test]
    fn test_basic_sequence() -> Result<(), Error> {
        let ok_seq = vec![
            EventType::Inception,
            EventType::Interaction,
            EventType::Interaction,
            EventType::Interaction,
            EventType::Rotation,
            EventType::Interaction,
        ];
        assert!(test_mock_event_sequence(ok_seq).is_ok());

        Ok(())
    }
}

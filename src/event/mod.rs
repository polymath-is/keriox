pub mod delegated;
pub mod inception;
pub mod interaction;
pub mod rotation;

use super::{
    error::Error,
    event_message::{EventMessage, SerializationFormats},
    prefix::IdentifierPrefix,
    state::{EventSemantics, IdentifierState},
};
use serde::{Deserialize, Serialize};
use serde_hex::{Compact, SerHex};

pub use self::{
    delegated::{DelegatedInceptionEvent, DelegatedRotationEvent},
    inception::InceptionEvent,
    interaction::InteractionEvent,
    rotation::RotationEvent,
};

/// Event Data
///
/// Event Data conveys the semantic content of a KERI event.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "ilk", rename_all = "lowercase")]
pub enum EventData {
    Icp(InceptionEvent),
    Rot(RotationEvent),
    Ixn(InteractionEvent),
    Dip(DelegatedInceptionEvent),
    Drt(DelegatedRotationEvent),
}

impl EventSemantics for EventData {
    fn apply_to(&self, state: IdentifierState) -> Result<IdentifierState, Error> {
        match self {
            Self::Icp(e) => e.apply_to(state),
            Self::Rot(e) => e.apply_to(state),
            Self::Ixn(e) => e.apply_to(state),
            Self::Dip(e) => e.apply_to(state),
            Self::Drt(e) => e.apply_to(state),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    #[serde(rename = "pre")]
    pub prefix: IdentifierPrefix,

    #[serde(with = "SerHex::<Compact>")]
    pub sn: u64,

    #[serde(flatten)]
    pub event_data: EventData,
}

impl Event {
    pub fn to_message(self, format: SerializationFormats) -> Result<EventMessage, Error> {
        EventMessage::new(self, format)
    }
}

impl EventSemantics for Event {
    fn apply_to(&self, state: IdentifierState) -> Result<IdentifierState, Error> {
        match self.event_data {
            EventData::Icp(_) => {
                // ICP events require the state to be uninitialized
                if state.prefix != IdentifierPrefix::default() || self.sn != 0 {
                    return Err(Error::SemanticError("SN is not correct".to_string()));
                }
            }
            _ => {
                // prefix must equal. sn must be incremented
                if self.prefix != state.prefix {
                    return Err(Error::SemanticError("Prefix does not match".to_string()));
                } else if self.sn != state.sn + 1 {
                    return Err(Error::SemanticError("SN is not correct".to_string()));
                }
            }
        };
        Ok(IdentifierState {
            sn: self.sn,
            prefix: self.prefix.clone(),
            ..self.event_data.apply_to(state)?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn ser_der() -> Result<(), serde_json::Error> {
        let event_str = "{
  \"pre\": \"DXq5YqaL6L48pf0fu7IUhL0JRaU2_RxFP0AL43wYn148\",
  \"sn\": \"0\",
  \"ilk\": \"icp\",
  \"sith\": \"2\",
  \"keys\":
  [
    \"BWoNZsa88VrTkep6HQt27fTh-4HA8tr54sHON1vWl6FE\",
    \"B8tr54sHON1vWVrTkep6H-4HAl6FEQt27fThWoNZsa88\",
    \"BVrTkep6HHA8tr54sHON1Qt27fThWoNZsa88-4vWl6FE\"
  ],
  \"nxt\": \"FWoNZsa88VrTkep6HQt27fTh-4HA8tr54sHON1vWl6FE\",
  \"toad\": \"2\",
  \"wits\":
  [
    \"DVrTkep6H-Qt27fThWoNZsa884HA8tr54sHON1vWl6FE\",
    \"DHON1vWl6FEQt27fThWoNZsa88VrTkep6H-4HA8tr54s\",
    \"DThWoNZsa88VrTkeQt27fp6H-4HA8tr54sHON1vWl6FE\"
  ],
  \"cnfg\": []
}";

        let event: Event = serde_json::from_str(event_str)?;

        print!("\n{}\n", serde_json::to_string(&event)?);

        Ok(())
    }
}

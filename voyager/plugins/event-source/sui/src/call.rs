use enumorph::Enumorph;
use macros::model;
use sui_sdk::types::event::EventID;
use unionlabs::primitives::H256;

#[model]
#[derive(Enumorph)]
#[allow(clippy::large_enum_variant)]
pub enum ModuleCall {
    FetchTransactions(FetchTransactions),
    FetchBlocks(FetchBlocks),
    FetchEvents(FetchEvents),
    MakeFullEvent(MakeFullEvent),
}

#[model]
pub struct FetchEvents {
    pub cursor: Option<EventID>,
}

#[model]
pub struct FetchBlocks {
    pub height: u64,
}

#[model]
pub struct FetchTransactions {
    pub height: u64,
}

#[model]
pub struct MakeFullEvent {
    pub event: crate::events::IbcEvent,
    pub tx_hash: H256,
    pub height: u64,
}

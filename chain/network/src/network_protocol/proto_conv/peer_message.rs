/// Conversion functions for PeerMessage - the top-level message for the NEAR P2P protocol format.
use super::*;

use crate::network_protocol::proto;
use crate::network_protocol::proto::peer_message::Message_type as ProtoMT;
use crate::network_protocol::{PeerMessage, RoutingTableUpdate, SyncAccountsData};
use crate::network_protocol::{RoutedMessage, RoutedMessageV2};
use crate::time::error::ComponentRange;
use borsh::{BorshDeserialize as _, BorshSerialize as _};
use near_primitives::block::{Block, BlockHeader};
use near_primitives::challenge::Challenge;
use near_primitives::syncing::{EpochSyncFinalizationResponse, EpochSyncResponse};
use near_primitives::transaction::SignedTransaction;
use near_primitives::types::EpochId;
use protobuf::MessageField as MF;
use std::sync::Arc;

#[derive(thiserror::Error, Debug)]
pub enum ParseRoutingTableUpdateError {
    #[error("edges {0}")]
    Edges(ParseVecError<ParseEdgeError>),
    #[error("accounts {0}")]
    Accounts(ParseVecError<ParseAnnounceAccountError>),
}

impl From<&RoutingTableUpdate> for proto::RoutingTableUpdate {
    fn from(x: &RoutingTableUpdate) -> Self {
        Self {
            edges: x.edges.iter().map(Into::into).collect(),
            accounts: x.accounts.iter().map(Into::into).collect(),
            ..Default::default()
        }
    }
}

impl TryFrom<&proto::RoutingTableUpdate> for RoutingTableUpdate {
    type Error = ParseRoutingTableUpdateError;
    fn try_from(x: &proto::RoutingTableUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            edges: try_from_slice(&x.edges).map_err(Self::Error::Edges)?,
            accounts: try_from_slice(&x.accounts).map_err(Self::Error::Accounts)?,
        })
    }
}

//////////////////////////////////////////

impl From<&BlockHeader> for proto::BlockHeader {
    fn from(x: &BlockHeader) -> Self {
        Self { borsh: x.try_to_vec().unwrap(), ..Default::default() }
    }
}

pub type ParseBlockHeaderError = borsh::maybestd::io::Error;

impl TryFrom<&proto::BlockHeader> for BlockHeader {
    type Error = ParseBlockHeaderError;
    fn try_from(x: &proto::BlockHeader) -> Result<Self, Self::Error> {
        Self::try_from_slice(&x.borsh)
    }
}

//////////////////////////////////////////

impl From<&Block> for proto::Block {
    fn from(x: &Block) -> Self {
        Self { borsh: x.try_to_vec().unwrap(), ..Default::default() }
    }
}

pub type ParseBlockError = borsh::maybestd::io::Error;

impl TryFrom<&proto::Block> for Block {
    type Error = ParseBlockError;
    fn try_from(x: &proto::Block) -> Result<Self, Self::Error> {
        Self::try_from_slice(&x.borsh)
    }
}

//////////////////////////////////////////

impl From<&PeerMessage> for proto::PeerMessage {
    fn from(x: &PeerMessage) -> Self {
        Self {
            message_type: Some(match x {
                PeerMessage::Handshake(h) => ProtoMT::Handshake(h.into()),
                PeerMessage::HandshakeFailure(pi, hfr) => {
                    ProtoMT::HandshakeFailure((pi, hfr).into())
                }
                PeerMessage::LastEdge(e) => ProtoMT::LastEdge(proto::LastEdge {
                    edge: MF::some(e.into()),
                    ..Default::default()
                }),
                PeerMessage::SyncRoutingTable(rtu) => ProtoMT::SyncRoutingTable(rtu.into()),
                PeerMessage::RequestUpdateNonce(pei) => {
                    ProtoMT::UpdateNonceRequest(proto::UpdateNonceRequest {
                        partial_edge_info: MF::some(pei.into()),
                        ..Default::default()
                    })
                }
                PeerMessage::ResponseUpdateNonce(e) => {
                    ProtoMT::UpdateNonceResponse(proto::UpdateNonceResponse {
                        edge: MF::some(e.into()),
                        ..Default::default()
                    })
                }
                PeerMessage::SyncAccountsData(msg) => {
                    ProtoMT::SyncAccountsData(proto::SyncAccountsData {
                        accounts_data: msg
                            .accounts_data
                            .iter()
                            .map(|d| d.as_ref().into())
                            .collect(),
                        incremental: msg.incremental,
                        requesting_full_sync: msg.requesting_full_sync,
                        ..Default::default()
                    })
                }
                PeerMessage::PeersRequest => ProtoMT::PeersRequest(proto::PeersRequest::new()),
                PeerMessage::PeersResponse(pis) => ProtoMT::PeersResponse(proto::PeersResponse {
                    peers: pis.iter().map(Into::into).collect(),
                    ..Default::default()
                }),
                PeerMessage::BlockHeadersRequest(bhs) => {
                    ProtoMT::BlockHeadersRequest(proto::BlockHeadersRequest {
                        block_hashes: bhs.iter().map(Into::into).collect(),
                        ..Default::default()
                    })
                }
                PeerMessage::BlockHeaders(bhs) => {
                    ProtoMT::BlockHeadersResponse(proto::BlockHeadersResponse {
                        block_headers: bhs.iter().map(Into::into).collect(),
                        ..Default::default()
                    })
                }
                PeerMessage::BlockRequest(bh) => ProtoMT::BlockRequest(proto::BlockRequest {
                    block_hash: MF::some(bh.into()),
                    ..Default::default()
                }),
                PeerMessage::Block(b) => ProtoMT::BlockResponse(proto::BlockResponse {
                    block: MF::some(b.into()),
                    ..Default::default()
                }),
                PeerMessage::Transaction(t) => ProtoMT::Transaction(proto::SignedTransaction {
                    borsh: t.try_to_vec().unwrap(),
                    ..Default::default()
                }),
                PeerMessage::Routed(r) => ProtoMT::Routed(proto::RoutedMessage {
                    borsh: r.msg.try_to_vec().unwrap(),
                    created_at: MF::from_option(r.created_at.as_ref().map(utc_to_proto)),
                    ..Default::default()
                }),
                PeerMessage::Disconnect => ProtoMT::Disconnect(proto::Disconnect::new()),
                PeerMessage::Challenge(r) => ProtoMT::Challenge(proto::Challenge {
                    borsh: r.try_to_vec().unwrap(),
                    ..Default::default()
                }),
                PeerMessage::EpochSyncRequest(epoch_id) => {
                    ProtoMT::EpochSyncRequest(proto::EpochSyncRequest {
                        epoch_id: MF::some((&epoch_id.0).into()),
                        ..Default::default()
                    })
                }
                PeerMessage::EpochSyncResponse(esr) => {
                    ProtoMT::EpochSyncResponse(proto::EpochSyncResponse {
                        borsh: esr.try_to_vec().unwrap(),
                        ..Default::default()
                    })
                }
                PeerMessage::EpochSyncFinalizationRequest(epoch_id) => {
                    ProtoMT::EpochSyncFinalizationRequest(proto::EpochSyncFinalizationRequest {
                        epoch_id: MF::some((&epoch_id.0).into()),
                        ..Default::default()
                    })
                }
                PeerMessage::EpochSyncFinalizationResponse(esfr) => {
                    ProtoMT::EpochSyncFinalizationResponse(proto::EpochSyncFinalizationResponse {
                        borsh: esfr.try_to_vec().unwrap(),
                        ..Default::default()
                    })
                }
            }),
            ..Default::default()
        }
    }
}

pub type ParseTransactionError = borsh::maybestd::io::Error;
pub type ParseRoutedError = borsh::maybestd::io::Error;
pub type ParseChallengeError = borsh::maybestd::io::Error;
pub type ParseEpochSyncResponseError = borsh::maybestd::io::Error;
pub type ParseEpochSyncFinalizationResponseError = borsh::maybestd::io::Error;

#[derive(thiserror::Error, Debug)]
pub enum ParsePeerMessageError {
    #[error("empty message")]
    Empty,
    #[error("handshake: {0}")]
    Handshake(ParseHandshakeError),
    #[error("handshake_failure: {0}")]
    HandshakeFailure(ParseHandshakeFailureError),
    #[error("last_edge: {0}")]
    LastEdge(ParseRequiredError<ParseEdgeError>),
    #[error("sync_routing_table: {0}")]
    SyncRoutingTable(ParseRoutingTableUpdateError),
    #[error("update_nonce_requrest: {0}")]
    UpdateNonceRequest(ParseRequiredError<ParsePartialEdgeInfoError>),
    #[error("update_nonce_response: {0}")]
    UpdateNonceResponse(ParseRequiredError<ParseEdgeError>),
    #[error("peers_response: {0}")]
    PeersResponse(ParseVecError<ParsePeerInfoError>),
    #[error("block_headers_request: {0}")]
    BlockHeadersRequest(ParseVecError<ParseCryptoHashError>),
    #[error("block_headers_response: {0}")]
    BlockHeadersResponse(ParseVecError<ParseBlockHeaderError>),
    #[error("block_request: {0}")]
    BlockRequest(ParseRequiredError<ParseCryptoHashError>),
    #[error("block_response: {0}")]
    BlockResponse(ParseRequiredError<ParseBlockError>),
    #[error("transaction: {0}")]
    Transaction(ParseTransactionError),
    #[error("routed: {0}")]
    Routed(ParseRoutedError),
    #[error("challenge: {0}")]
    Challenge(ParseChallengeError),
    #[error("epoch_sync_request: {0}")]
    EpochSyncRequest(ParseRequiredError<ParseCryptoHashError>),
    #[error("epoch_sync_response: {0}")]
    EpochSyncResponse(ParseEpochSyncResponseError),
    #[error("epoch_sync_finalization_request: {0}")]
    EpochSyncFinalizationRequest(ParseRequiredError<ParseCryptoHashError>),
    #[error("epoch_sync_finalization_response: {0}")]
    EpochSyncFinalizationResponse(ParseEpochSyncFinalizationResponseError),
    #[error("routed_created_at: {0}")]
    RoutedCreatedAtTimestamp(ComponentRange),
    #[error("sync_accounts_data: {0}")]
    SyncAccountsData(ParseVecError<ParseSignedAccountDataError>),
}

impl TryFrom<&proto::PeerMessage> for PeerMessage {
    type Error = ParsePeerMessageError;
    fn try_from(x: &proto::PeerMessage) -> Result<Self, Self::Error> {
        Ok(match x.message_type.as_ref().ok_or(Self::Error::Empty)? {
            ProtoMT::Handshake(h) => {
                PeerMessage::Handshake(h.try_into().map_err(Self::Error::Handshake)?)
            }
            ProtoMT::HandshakeFailure(hf) => {
                let (pi, hfr) = hf.try_into().map_err(Self::Error::HandshakeFailure)?;
                PeerMessage::HandshakeFailure(pi, hfr)
            }
            ProtoMT::LastEdge(le) => {
                PeerMessage::LastEdge(try_from_required(&le.edge).map_err(Self::Error::LastEdge)?)
            }
            ProtoMT::SyncRoutingTable(rtu) => PeerMessage::SyncRoutingTable(
                rtu.try_into().map_err(Self::Error::SyncRoutingTable)?,
            ),
            ProtoMT::UpdateNonceRequest(unr) => PeerMessage::RequestUpdateNonce(
                try_from_required(&unr.partial_edge_info)
                    .map_err(Self::Error::UpdateNonceRequest)?,
            ),
            ProtoMT::UpdateNonceResponse(unr) => PeerMessage::ResponseUpdateNonce(
                try_from_required(&unr.edge).map_err(Self::Error::UpdateNonceResponse)?,
            ),
            ProtoMT::SyncAccountsData(msg) => PeerMessage::SyncAccountsData(SyncAccountsData {
                accounts_data: try_from_slice(&msg.accounts_data)
                    .map_err(Self::Error::SyncAccountsData)?
                    .into_iter()
                    .map(Arc::new)
                    .collect(),
                incremental: msg.incremental,
                requesting_full_sync: msg.requesting_full_sync,
            }),
            ProtoMT::PeersRequest(_) => PeerMessage::PeersRequest,
            ProtoMT::PeersResponse(pr) => PeerMessage::PeersResponse(
                try_from_slice(&pr.peers).map_err(Self::Error::PeersResponse)?,
            ),
            ProtoMT::BlockHeadersRequest(bhr) => PeerMessage::BlockHeadersRequest(
                try_from_slice(&bhr.block_hashes).map_err(Self::Error::BlockHeadersRequest)?,
            ),
            ProtoMT::BlockHeadersResponse(bhr) => PeerMessage::BlockHeaders(
                try_from_slice(&bhr.block_headers).map_err(Self::Error::BlockHeadersResponse)?,
            ),
            ProtoMT::BlockRequest(br) => PeerMessage::BlockRequest(
                try_from_required(&br.block_hash).map_err(Self::Error::BlockRequest)?,
            ),
            ProtoMT::BlockResponse(br) => PeerMessage::Block(
                try_from_required(&br.block).map_err(Self::Error::BlockResponse)?,
            ),
            ProtoMT::Transaction(t) => PeerMessage::Transaction(
                SignedTransaction::try_from_slice(&t.borsh).map_err(Self::Error::Transaction)?,
            ),
            ProtoMT::Routed(r) => PeerMessage::Routed(Box::new(RoutedMessageV2 {
                msg: RoutedMessage::try_from_slice(&r.borsh).map_err(Self::Error::Routed)?,
                created_at: r
                    .created_at
                    .as_ref()
                    .map(utc_from_proto)
                    .transpose()
                    .map_err(Self::Error::RoutedCreatedAtTimestamp)?,
            })),
            ProtoMT::Disconnect(_) => PeerMessage::Disconnect,
            ProtoMT::Challenge(c) => PeerMessage::Challenge(
                Challenge::try_from_slice(&c.borsh).map_err(Self::Error::Challenge)?,
            ),
            ProtoMT::EpochSyncRequest(esr) => PeerMessage::EpochSyncRequest(EpochId(
                try_from_required(&esr.epoch_id).map_err(Self::Error::EpochSyncRequest)?,
            )),
            ProtoMT::EpochSyncResponse(esr) => PeerMessage::EpochSyncResponse(Box::new(
                EpochSyncResponse::try_from_slice(&esr.borsh)
                    .map_err(Self::Error::EpochSyncResponse)?,
            )),
            ProtoMT::EpochSyncFinalizationRequest(esr) => {
                PeerMessage::EpochSyncFinalizationRequest(EpochId(
                    try_from_required(&esr.epoch_id)
                        .map_err(Self::Error::EpochSyncFinalizationRequest)?,
                ))
            }
            ProtoMT::EpochSyncFinalizationResponse(esr) => {
                PeerMessage::EpochSyncFinalizationResponse(Box::new(
                    EpochSyncFinalizationResponse::try_from_slice(&esr.borsh)
                        .map_err(Self::Error::EpochSyncFinalizationResponse)?,
                ))
            }
        })
    }
}

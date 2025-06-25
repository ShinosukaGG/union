use std::{cmp::Ordering, collections::VecDeque};

use ibc_union_spec::{
    event::{
        ChannelMetadata, ChannelOpenAck, ChannelOpenConfirm, ChannelOpenInit, ChannelOpenTry,
        ConnectionMetadata, ConnectionOpenAck, ConnectionOpenConfirm, ConnectionOpenInit,
        ConnectionOpenTry, CreateClient, FullEvent, PacketMetadata, PacketSend, UpdateClient,
    },
    path::{ChannelPath, ConnectionPath},
    ChannelId, ClientId, IbcUnion, Timestamp,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    types::ErrorObject,
    Extensions,
};
use serde::{Deserialize, Serialize};
use sui_sdk::{
    rpc_types::SuiTransactionBlockResponseOptions, types::base_types::SuiAddress, SuiClientBuilder,
};
use tracing::{info, instrument};
use unionlabs::{ibc::core::client::height::Height, primitives::H256, ErrorReporter};
use voyager_sdk::{
    hook::simple_take_filter,
    message::{
        call::{Call, WaitForHeight},
        data::{ChainEvent, Data, EventProvableHeight},
        PluginMessage, VoyagerMessage,
    },
    plugin::Plugin,
    primitives::{ChainId, ClientInfo, ClientType, QueryHeight},
    rpc::{types::PluginInfo, PluginServer},
    vm::{
        call, conc, data,
        pass::{PassResult, Ready},
        seq, Op,
    },
    DefaultCmd, ExtensionsExt, VoyagerClient,
};

use crate::{
    call::{FetchBlocks, FetchTransactions, MakeFullEvent, ModuleCall},
    callback::ModuleCallback,
};

pub mod call;
pub mod callback;
pub mod data;

pub mod events;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    Module::run().await
}

#[derive(clap::Subcommand)]
pub enum Cmd {
    ChainId,
    VaultAddress,
    SubmitTx,
    FetchAbi,
}

#[derive(Clone)]
pub struct Module {
    pub chain_id: ChainId,

    pub sui_client: sui_sdk::SuiClient,

    pub ibc_handler_address: SuiAddress,
}

impl Plugin for Module {
    type Call = ModuleCall;
    type Callback = ModuleCallback;

    type Config = Config;
    type Cmd = DefaultCmd;

    async fn new(config: Self::Config) -> anyhow::Result<Self> {
        let sui_client = SuiClientBuilder::default().build(&config.rpc_url).await?;

        let chain_id = sui_client.read_api().get_chain_identifier().await?;

        Ok(Self {
            chain_id: ChainId::new(chain_id.to_string()),
            sui_client,
            ibc_handler_address: config.ibc_handler_address,
        })
    }

    fn info(config: Self::Config) -> PluginInfo {
        PluginInfo {
            name: plugin_name(&config.chain_id),
            interest_filter: simple_take_filter(format!(
                r#"[.. | (."@type"? == "index" or ."@type"? == "index_range") and ."@value".chain_id == "{}"] | any"#,
                config.chain_id
            )),
        }
    }

    async fn cmd(_config: Self::Config, cmd: Self::Cmd) {
        match cmd {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub chain_id: ChainId,
    pub rpc_url: String,
    pub ibc_handler_address: SuiAddress,
}

fn plugin_name(chain_id: &ChainId) -> String {
    pub const PLUGIN_NAME: &str = env!("CARGO_PKG_NAME");

    format!("{PLUGIN_NAME}/{}", chain_id)
}

impl Module {
    fn plugin_name(&self) -> String {
        plugin_name(&self.chain_id)
    }

    async fn fetch_blocks(
        &self,
        voyager_client: &VoyagerClient,
        height: u64,
    ) -> RpcResult<Op<VoyagerMessage>> {
        Ok(conc([
            call(PluginMessage::new(
                self.plugin_name(),
                ModuleCall::from(FetchTransactions { height }),
            )),
            {
                let latest_height = voyager_client
                    .query_latest_height(self.chain_id.clone(), true)
                    .await?
                    .height();

                match latest_height.cmp(&latest_height) {
                    Ordering::Less => {
                        let next_height = (latest_height - height).clamp(1, 20) + height;
                        conc(
                            ((height + 1)..next_height)
                                .map(|height| {
                                    call(PluginMessage::new(
                                        self.plugin_name(),
                                        ModuleCall::from(FetchTransactions { height }),
                                    ))
                                })
                                .chain([call(PluginMessage::new(
                                    self.plugin_name(),
                                    ModuleCall::from(FetchBlocks {
                                        height: next_height,
                                    }),
                                ))]),
                        )
                    }
                    Ordering::Equal | Ordering::Greater => seq([
                        call(WaitForHeight {
                            chain_id: self.chain_id.clone(),
                            height: Height::new(height + 1),
                            finalized: true,
                        }),
                        call(PluginMessage::new(
                            self.plugin_name(),
                            ModuleCall::from(FetchBlocks { height: height + 1 }),
                        )),
                    ]),
                }
            },
        ]))
    }

    async fn make_packet_metadata(
        &self,
        event_height: Height,
        self_channel_id: ChannelId,
        voyager_client: &VoyagerClient,
    ) -> RpcResult<(ChainId, ClientInfo, ChannelMetadata, ChannelMetadata)> {
        let self_channel = voyager_client
            .query_ibc_state(
                self.chain_id.clone(),
                QueryHeight::Specific(event_height),
                ChannelPath {
                    channel_id: self_channel_id,
                },
            )
            .await?;

        let self_connection_id = self_channel.connection_id;
        let self_connection = voyager_client
            .query_ibc_state(
                self.chain_id.clone(),
                QueryHeight::Specific(event_height),
                ConnectionPath {
                    connection_id: self_connection_id,
                },
            )
            .await?;

        let client_info = voyager_client
            .client_info::<IbcUnion>(self.chain_id.clone(), self_connection.client_id)
            .await?;

        let client_state_meta = voyager_client
            .client_state_meta::<IbcUnion>(
                self.chain_id.clone(),
                event_height.into(),
                self_connection.client_id,
            )
            .await?;

        let counterparty_latest_height = voyager_client
            .query_latest_height(client_state_meta.counterparty_chain_id.clone(), false)
            .await?;

        let other_channel_id = self_channel.counterparty_channel_id.unwrap();

        let other_channel = voyager_client
            .query_ibc_state(
                client_state_meta.counterparty_chain_id.clone(),
                QueryHeight::Specific(counterparty_latest_height),
                ChannelPath {
                    channel_id: other_channel_id,
                },
            )
            .await?;

        let self_channel = ChannelMetadata {
            channel_id: self_channel_id,
            version: self_channel.version,
            connection: ConnectionMetadata {
                client_id: self_connection.client_id,
                connection_id: self_connection_id,
            },
        };
        let other_channel = ChannelMetadata {
            channel_id: other_channel_id,
            version: other_channel.version,
            connection: ConnectionMetadata {
                client_id: self_connection.counterparty_client_id,
                connection_id: self_connection.counterparty_connection_id.unwrap(),
            },
        };

        Ok((
            client_state_meta.counterparty_chain_id,
            client_info,
            self_channel,
            other_channel,
        ))
    }
}

#[async_trait]
impl PluginServer<ModuleCall, ModuleCallback> for Module {
    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn run_pass(
        &self,
        _: &Extensions,
        msgs: Vec<Op<VoyagerMessage>>,
    ) -> RpcResult<PassResult<VoyagerMessage>> {
        Ok(PassResult {
            optimize_further: vec![],
            ready: msgs
                .into_iter()
                .map(|op| match op {
                    Op::Call(Call::Index(fetch)) if fetch.chain_id == self.chain_id => {
                        call(PluginMessage::new(
                            self.plugin_name(),
                            ModuleCall::FetchBlocks(FetchBlocks {
                                height: fetch.start_height.height(),
                            }),
                        ))
                    }
                    op => op,
                })
                .enumerate()
                .map(|(i, op)| Ready::new(vec![i], op))
                .collect(),
        })
    }

    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn callback(
        &self,
        _: &Extensions,
        cb: ModuleCallback,
        _data: VecDeque<Data>,
    ) -> RpcResult<Op<VoyagerMessage>> {
        match cb {}
    }

    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn call(&self, e: &Extensions, msg: ModuleCall) -> RpcResult<Op<VoyagerMessage>> {
        match msg {
            ModuleCall::FetchBlocks(FetchBlocks { height }) => {
                self.fetch_blocks(e.voyager_client()?, height).await
            }
            ModuleCall::FetchTransactions(FetchTransactions { height }) => {
                info!("fetching block height {height}");

                let tx_digests = self
                    .sui_client
                    .read_api()
                    .get_checkpoint(sui_sdk::rpc_types::CheckpointId::SequenceNumber(height))
                    .await
                    .map_err(|e| {
                        ErrorObject::owned(
                            -1,
                            ErrorReporter(e).with_message("error fetching a checkpoint"),
                            None::<()>,
                        )
                    })?
                    .transactions;

                let events = self
                    .sui_client
                    .read_api()
                    .multi_get_transactions_with_options(
                        tx_digests,
                        SuiTransactionBlockResponseOptions::new().with_events(),
                    )
                    .await
                    .map_err(|e| {
                        ErrorObject::owned(
                            -1,
                            ErrorReporter(e).with_message("error fetching txs"),
                            None::<()>,
                        )
                    })?
                    .into_iter()
                    .flat_map(|tx| {
                        tx.events
                            .expect("events exist")
                            .data
                            .into_iter()
                            .map(move |events| (events, tx.digest))
                    })
                    .filter_map(|(e, hash)| {
                        (e.type_.address == self.ibc_handler_address.into()).then_some((e, hash))
                    })
                    .map(|(e, hash)| {
                        println!("event: {e:?}");
                        let event = match e.type_.name.as_str() {
                            "CreateClient" => {
                                let create_client: events::CreateClient =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::CreateClient(create_client)
                            }
                            "UpdateClient" => {
                                let update_client: events::UpdateClient =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::UpdateClient(update_client)
                            }
                            "ConnectionOpenInit" => {
                                let connection_open: events::ConnectionOpenInit =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ConnectionOpenInit(connection_open)
                            }
                            "ConnectionOpenTry" => {
                                let connection_open: events::ConnectionOpenTry =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ConnectionOpenTry(connection_open)
                            }
                            "ConnectionOpenAck" => {
                                let connection_open: events::ConnectionOpenAck =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ConnectionOpenAck(connection_open)
                            }
                            "ConnectionOpenConfirm" => {
                                let connection_open: events::ConnectionOpenConfirm =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ConnectionOpenConfirm(connection_open)
                            }
                            "ChannelOpenInit" => {
                                let channel_open: events::ChannelOpenInit =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ChannelOpenInit(channel_open)
                            }
                            "ChannelOpenTry" => {
                                let channel_open: events::ChannelOpenTry =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ChannelOpenTry(channel_open)
                            }
                            "ChannelOpenAck" => {
                                let channel_open: events::ChannelOpenAck =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ChannelOpenAck(channel_open)
                            }
                            "ChannelOpenConfirm" => {
                                let channel_open: events::ChannelOpenConfirm =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::ChannelOpenConfirm(channel_open)
                            }
                            "PacketSend" => {
                                let channel_open: events::PacketSend =
                                    serde_json::from_value(e.parsed_json).unwrap();
                                events::IbcEvent::PacketSend(channel_open)
                            }
                            e => panic!("unknown: {e}"),
                        };

                        info!("found event: {event:?}");
                        call(PluginMessage::new(
                            self.plugin_name(),
                            ModuleCall::from(MakeFullEvent {
                                event,
                                tx_hash: H256::new(hash.into_inner()),
                                height,
                            }),
                        ))
                    });

                Ok(conc(events))
            }
            ModuleCall::MakeFullEvent(MakeFullEvent {
                event,
                tx_hash,
                height,
            }) => {
                let (full_event, client_id): (FullEvent, ClientId) = match event {
                    events::IbcEvent::CreateClient(event) => (
                        CreateClient {
                            client_type: ClientType::new(event.client_type),
                            client_id: event.client_id.try_into().unwrap(),
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::UpdateClient(event) => (
                        UpdateClient {
                            client_type: ClientType::new(event.client_type),
                            client_id: event.client_id.try_into().unwrap(),
                            height: event.height.0,
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::ConnectionOpenInit(event) => (
                        ConnectionOpenInit {
                            connection_id: event.connection_id.try_into().unwrap(),
                            client_id: ClientId::new(event.client_id.try_into().unwrap()),
                            counterparty_client_id: event
                                .counterparty_client_id
                                .try_into()
                                .unwrap(),
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::ConnectionOpenTry(event) => (
                        ConnectionOpenTry {
                            client_id: event.client_id.try_into().unwrap(),
                            connection_id: event.connection_id.try_into().unwrap(),
                            counterparty_client_id: event
                                .counterparty_client_id
                                .try_into()
                                .unwrap(),
                            counterparty_connection_id: event
                                .counterparty_connection_id
                                .try_into()
                                .unwrap(),
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::ConnectionOpenAck(event) => (
                        ConnectionOpenAck {
                            client_id: event.client_id.try_into().unwrap(),
                            connection_id: event.connection_id.try_into().unwrap(),
                            counterparty_client_id: event
                                .counterparty_client_id
                                .try_into()
                                .unwrap(),
                            counterparty_connection_id: event
                                .counterparty_connection_id
                                .try_into()
                                .unwrap(),
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::ConnectionOpenConfirm(event) => (
                        ConnectionOpenConfirm {
                            client_id: event.client_id.try_into().unwrap(),
                            connection_id: event.connection_id.try_into().unwrap(),
                            counterparty_client_id: event
                                .counterparty_client_id
                                .try_into()
                                .unwrap(),
                            counterparty_connection_id: event
                                .counterparty_connection_id
                                .try_into()
                                .unwrap(),
                        }
                        .into(),
                        event.client_id.try_into().unwrap(),
                    ),
                    events::IbcEvent::ChannelOpenInit(event) => {
                        let voyager_client = e.voyager_client()?;
                        let connection = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ConnectionPath {
                                    connection_id: event.connection_id.try_into().unwrap(),
                                },
                            )
                            .await?;

                        let client_id = connection.client_id;

                        (
                            ChannelOpenInit {
                                port_id: event.port_id.into_bytes().into(),
                                channel_id: event.channel_id.try_into().unwrap(),
                                counterparty_port_id: event.counterparty_port_id.into(),
                                connection,
                                version: event.version,
                            }
                            .into(),
                            client_id,
                        )
                    }
                    events::IbcEvent::ChannelOpenTry(event) => {
                        let voyager_client = e.voyager_client()?;
                        let connection = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ConnectionPath {
                                    connection_id: event.connection_id.try_into().unwrap(),
                                },
                            )
                            .await?;

                        let client_id = connection.client_id;
                        (
                            ChannelOpenTry {
                                port_id: event.port_id.into_bytes().into(),
                                channel_id: event.channel_id.try_into().unwrap(),
                                counterparty_port_id: event.counterparty_port_id.into(),
                                counterparty_channel_id: event
                                    .counterparty_channel_id
                                    .try_into()
                                    .unwrap(),
                                connection,
                                version: event.version,
                            }
                            .into(),
                            client_id,
                        )
                    }
                    events::IbcEvent::ChannelOpenAck(event) => {
                        let voyager_client = e.voyager_client()?;
                        let connection = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ConnectionPath {
                                    connection_id: event.connection_id.try_into().unwrap(),
                                },
                            )
                            .await?;
                        let channel = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ChannelPath {
                                    channel_id: event.channel_id.try_into().unwrap(),
                                },
                            )
                            .await?;

                        let client_id = connection.client_id;
                        (
                            ChannelOpenAck {
                                port_id: event.port_id.into_bytes().into(),
                                channel_id: event.channel_id.try_into().unwrap(),
                                counterparty_port_id: event.counterparty_port_id.into(),
                                counterparty_channel_id: event
                                    .counterparty_channel_id
                                    .try_into()
                                    .unwrap(),
                                connection,
                                version: channel.version, // version: event.version,
                            }
                            .into(),
                            client_id,
                        )
                    }
                    events::IbcEvent::ChannelOpenConfirm(event) => {
                        let voyager_client = e.voyager_client()?;
                        let connection = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ConnectionPath {
                                    connection_id: event.connection_id.try_into().unwrap(),
                                },
                            )
                            .await?;
                        let channel = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ChannelPath {
                                    channel_id: event.channel_id.try_into().unwrap(),
                                },
                            )
                            .await?;

                        let client_id = connection.client_id;
                        (
                            ChannelOpenConfirm {
                                port_id: event.port_id.into_bytes().into(),
                                channel_id: event.channel_id.try_into().unwrap(),
                                counterparty_port_id: event.counterparty_port_id.into(),
                                counterparty_channel_id: event
                                    .counterparty_channel_id
                                    .try_into()
                                    .unwrap(),
                                connection,
                                version: channel.version,
                            }
                            .into(),
                            client_id,
                        )
                    }
                    events::IbcEvent::PacketSend(event) => {
                        let packet: events::Packet = event.packet;

                        let voyager_client = e.voyager_client()?;
                        let channel = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ChannelPath {
                                    channel_id: packet.source_channel_id.try_into().unwrap(),
                                },
                            )
                            .await?;

                        let connection = voyager_client
                            .query_ibc_state(
                                self.chain_id.clone(),
                                QueryHeight::Specific(Height::new(height)),
                                ibc_union_spec::path::ConnectionPath {
                                    connection_id: channel.connection_id,
                                },
                            )
                            .await?;

                        let client_id = connection.client_id;

                        let (
                            _counterparty_chain_id,
                            _client_info,
                            source_channel,
                            destination_channel,
                        ) = self
                            .make_packet_metadata(
                                Height::new(height),
                                packet.source_channel_id.try_into().unwrap(),
                                voyager_client,
                            )
                            .await?;
                        (
                            PacketSend {
                                packet_data: packet.data.into(),
                                packet: PacketMetadata {
                                    source_channel,
                                    destination_channel,
                                    timeout_height: packet.timeout_height.0,
                                    timeout_timestamp: Timestamp::from_nanos(
                                        packet.timeout_timestamp.0,
                                    ),
                                },
                            }
                            .into(),
                            client_id,
                        )
                    }
                };
                ibc_union_spec::log_event(&full_event, &self.chain_id);

                let voyager_client = e.voyager_client()?;

                let client_info = voyager_client
                    .client_info::<IbcUnion>(self.chain_id.clone(), client_id)
                    .await?;

                let client_state_meta = voyager_client
                    .client_state_meta::<IbcUnion>(
                        self.chain_id.clone(),
                        Height::new(height).into(),
                        client_id,
                    )
                    .await?;

                Ok(data(ChainEvent::new::<IbcUnion>(
                    self.chain_id.clone(),
                    client_info,
                    client_state_meta.counterparty_chain_id,
                    tx_hash,
                    EventProvableHeight::Exactly(Height::new(height)),
                    full_event,
                )))
            }
        }
    }
}

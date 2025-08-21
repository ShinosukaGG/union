use std::error::Error;

use alloy_sol_types::SolType;
use ucs03_zkgm::com::{TAG_ACK_FAILURE, TAG_ACK_SUCCESS};
use unionlabs_primitives::{Bytes, H256, U256};

pub use crate::{batch::Batch, call::Call, forward::Forward, root::Root, token_order::TokenOrder};
use crate::{
    batch::BatchAck,
    call::CallAck,
    root::RootShape,
    token_order::{TokenOrderAck, TokenOrderShape},
};

pub mod batch;
pub mod call;
pub mod forward;
pub mod root;
pub mod stake;
pub mod token_order;
pub mod unstake;
pub mod withdraw_rewards;
pub mod withdraw_stake;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZkgmPacket {
    salt: H256,
    path: U256,
    instruction: Root,
}

impl ZkgmPacket {
    pub fn decode(bz: impl AsRef<[u8]>) -> Result<Self> {
        let ucs03_zkgm::com::ZkgmPacket {
            salt,
            path,
            instruction,
        } = ucs03_zkgm::com::ZkgmPacket::abi_decode_params_validate(bz.as_ref())?;

        Ok(Self {
            salt: salt.into(),
            path: path.into(),
            instruction: Root::from_raw(instruction)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ack {
    Success(RootAck),
    Failure(Bytes),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootAck {
    Batch(BatchAck),
    TokenOrder(TokenOrderAck),
    Call(CallAck),
}

impl Ack {
    pub fn decode(shape: RootShape, bz: impl AsRef<[u8]>) -> Result<Self> {
        let ucs03_zkgm::com::Ack { tag, inner_ack } =
            ucs03_zkgm::com::Ack::abi_decode_params_validate(bz.as_ref())?;

        match tag {
            TAG_ACK_SUCCESS => match shape {
                RootShape::Batch(shape) => BatchAck::decode(shape, inner_ack).map(RootAck::Batch),
                RootShape::TokenOrder(shape) => {
                    TokenOrderAck::decode(shape, inner_ack).map(RootAck::TokenOrder)
                }
                RootShape::Call(shape) => CallAck::decode(shape, inner_ack).map(RootAck::Call),
                RootShape::Forward(_shape) => todo!(),
                RootShape::Stake(_shape) => todo!(),
                RootShape::Unstake(_shape) => todo!(),
                RootShape::WithdrawStake(_shape) => todo!(),
                RootShape::WithdrawRewards(_shape) => todo!(),
            }
            .map(Ack::Success),
            TAG_ACK_FAILURE => Ok(Ack::Failure(inner_ack.into())),
            invalid => Err(format!("invalid ack tag {invalid}"))?,
        }
    }
}

// pub mod abi {
//     alloy_sol_types::sol! {
//         "../../evm/contracts/apps/ucs/03-zkgm/Types.sol"
//     }
// }

// pub mod zkgm_lib_abi {
//     alloy_sol_types::sol! {
//         bytes public constant ACK_EMPTY = "";

//         uint256 public constant ACK_FAILURE = 0x00;
//         uint256 public constant ACK_SUCCESS = 0x01;

//         bytes public constant ACK_ERR_ONLYMAKER = "DEADC0DE";

//         bytes32 public constant ACK_ERR_ONLYMAKER_HASH =
//             keccak256(ACK_ERR_ONLYMAKER);

//         uint256 public constant FILL_TYPE_PROTOCOL = 0xB0CAD0;
//         uint256 public constant FILL_TYPE_MARKETMAKER = 0xD1CEC45E;

//         uint8 public constant TOKEN_ORDER_KIND_INITIALIZE = 0x00;
//         uint8 public constant TOKEN_ORDER_KIND_ESCROW = 0x01;
//         uint8 public constant TOKEN_ORDER_KIND_UNESCROW = 0x02;

//         // Public instructions
//         uint8 public constant OP_FORWARD = 0x00;
//         uint8 public constant OP_CALL = 0x01;
//         uint8 public constant OP_BATCH = 0x02;
//         uint8 public constant OP_TOKEN_ORDER = 0x03;

//         uint8 public constant OP_STAKE = 0x04;
//         uint8 public constant OP_UNSTAKE = 0x05;
//         uint8 public constant OP_WITHDRAW_STAKE = 0x06;
//         uint8 public constant OP_WITHDRAW_REWARDS = 0x07;

//         uint8 public constant WRAPPED_TOKEN_KIND_PROTOCOL = 0x00;
//         uint8 public constant WRAPPED_TOKEN_KIND_THIRD_PARTY = 0x01;

//         uint8 public constant INSTR_VERSION_0 = 0x00;
//         uint8 public constant INSTR_VERSION_1 = 0x01;
//         uint8 public constant INSTR_VERSION_2 = 0x02;

//         bytes32 public constant FORWARD_SALT_MAGIC =
//             0xC0DE00000000000000000000000000000000000000000000000000000000BABE;

//         address public constant NATIVE_TOKEN_ERC_7528_ADDRESS =
//             0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE;

//         string public constant IBC_VERSION_STR = "ucs03-zkgm-0";
//         bytes32 public constant IBC_VERSION = keccak256(bytes(IBC_VERSION_STR));
//     }
// }

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;
    use crate::{
        batch::{
            BatchV0, BatchV0Ack, BatchV0Shape, BatchableInstructionV0, BatchableInstructionV0Ack,
            BatchableInstructionV0Shape,
        },
        call::{CallAckV0, CallShape, CallV0, CallV0Shape},
        token_order::{TokenOrderV1, TokenOrderV1Ack},
    };

    #[test]
    fn decode() {
        let packet = hex!("79176e1d5f2779e14b2f5f885bfe7b35e78802643522ce0dad5cac4e4a44271f00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000066000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000003a000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000002e00000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000002710000000000000000000000000000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000002600000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000002a00000000000000000000000000000000000000000000000000000000000002710000000000000000000000000000000000000000000000000000000000000001415ee7c367f4232241028c36e720803100757c6e9000000000000000000000000000000000000000000000000000000000000000000000000000000000000003e62626e316d377a72356a77346b397a32327239616a676766347563616c7779377578767539676b7736746e736d7634326c766a706b7761736167656b356700000000000000000000000000000000000000000000000000000000000000000014e53dcec07d16d88e386ae0710e86d9a400f83c31000000000000000000000000000000000000000000000000000000000000000000000000000000000000000442414259000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007426162796c6f6e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000047562626e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000001415ee7c367f4232241028c36e720803100757c6e9000000000000000000000000000000000000000000000000000000000000000000000000000000000000003e62626e316d377a72356a77346b397a32327239616a676766347563616c7779377578767539676b7736746e736d7634326c766a706b7761736167656b3567000000000000000000000000000000000000000000000000000000000000000000b27b22626f6e64223a7b22616d6f756e74223a223130303030222c2273616c74223a22307833313333303831396135613232336439376163373134663239616535653361646265396565663833383233373830663761393063636536363461626138366565222c226578706563746564223a2239373237222c22726563697069656e74223a2262626e3168637533306461647770686638397533783375366a327a35387233376339616b687866637330227d7d0000000000000000000000000000");

        let ack = hex!("00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000b0cad00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001");

        let expected_packet = ZkgmPacket {
            salt: hex!(
                "79176e1d5f2779e14b2f5f885bfe7b35e78802643522ce0dad5cac4e4a44271f"
            ).into(),
            path: U256::ZERO,
            instruction: Root::Batch(Batch::V0(BatchV0 {
                instructions: vec![
                    BatchableInstructionV0::TokenOrder(TokenOrder::V1(TokenOrderV1 {
                        sender: hex!("15ee7c367f4232241028c36e720803100757c6e9").into(),
                        receiver: b"bbn1m7zr5jw4k9z22r9ajggf4ucalwy7uxvu9gkw6tnsmv42lvjpkwasagek5g"
                            .into(),
                        base_token: hex!("e53dcec07d16d88e386ae0710e86d9a400f83c31").into(),
                        base_amount: U256::from(10000_u64),
                        base_token_symbol: "BABY".to_owned(),
                        base_token_name: "Babylon".to_owned(),
                        base_token_decimals: 6,
                        base_token_path: U256::from(1_u64),
                        quote_token: b"ubbn".into(),
                        quote_amount: U256::from(10000_u64),
                    })),
                    BatchableInstructionV0::Call(Call::V0(CallV0 {
                        sender: hex!("15ee7c367f4232241028c36e720803100757c6e9").into(),
                        eureka: false,
                        contract_address:
                            b"bbn1m7zr5jw4k9z22r9ajggf4ucalwy7uxvu9gkw6tnsmv42lvjpkwasagek5g".into(),
                        contract_calldata: br#"{"bond":{"amount":"10000","salt":"0x31330819a5a223d97ac714f29ae5e3adbe9eef83823780f7a90cce664aba86ee","expected":"9727","recipient":"bbn1hcu30dadwphf89u3x3u6j2z58r37c9akhxfcs0"}}"#.into()
                    })),
                ],
            })),
        };

        let decoded_packet = ZkgmPacket::decode(packet).unwrap();

        assert_eq!(decoded_packet, expected_packet);

        let expected_shape = RootShape::Batch(batch::BatchShape::V0(BatchV0Shape {
            instructions: vec![
                BatchableInstructionV0Shape::TokenOrder(TokenOrderShape::V1),
                BatchableInstructionV0Shape::Call(CallShape::V0(CallV0Shape { eureka: false })),
            ],
        }));

        let shape = decoded_packet.instruction.shape();

        assert_eq!(shape, expected_shape);

        let ack = Ack::decode(decoded_packet.instruction.shape(), ack).unwrap();

        let expected_ack = Ack::Success(RootAck::Batch(BatchAck::V0(BatchV0Ack {
            instructions: vec![
                BatchableInstructionV0Ack::TokenOrder(TokenOrderAck::V1(TokenOrderV1Ack::Protocol)),
                BatchableInstructionV0Ack::Call(CallAck::V0(CallAckV0::NonEureka)),
            ],
        })));

        assert_eq!(ack, expected_ack);
    }
}

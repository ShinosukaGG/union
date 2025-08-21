use alloy_sol_types::SolType;
use enumorph::Enumorph;
use ucs03_zkgm::com::{
    INSTR_VERSION_0, OP_CALL, OP_STAKE, OP_TOKEN_ORDER, OP_UNSTAKE, OP_WITHDRAW_REWARDS,
    OP_WITHDRAW_STAKE,
};

use crate::{
    call::{Call, CallAck, CallShape},
    stake::{Stake, StakeShape},
    token_order::{TokenOrder, TokenOrderAck, TokenOrderShape},
    unstake::{Unstake, UnstakeShape},
    withdraw_rewards::{WithdrawRewards, WithdrawRewardsShape},
    withdraw_stake::{WithdrawStake, WithdrawStakeShape},
    Result,
};

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum Batch {
    V0(BatchV0),
}

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum BatchShape {
    V0(BatchV0Shape),
}

impl Batch {
    pub(crate) fn decode(version: u8, operand: impl AsRef<[u8]>) -> Result<Self> {
        match version {
            INSTR_VERSION_0 => BatchV0::decode(operand).map(Into::into),
            invalid => Err(format!("invalid batch version: {invalid}"))?,
        }
    }

    pub(crate) fn shape(&self) -> BatchShape {
        match self {
            Batch::V0(batch_v0) => BatchShape::V0(BatchV0Shape {
                instructions: batch_v0.instructions.iter().map(|b| b.shape()).collect(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum BatchAck {
    V0(BatchV0Ack),
}

impl BatchAck {
    pub(crate) fn decode(shape: BatchShape, ack: impl AsRef<[u8]>) -> Result<Self> {
        match shape {
            BatchShape::V0(BatchV0Shape { instructions }) => {
                let ucs03_zkgm::com::BatchAck { acknowledgements } =
                    ucs03_zkgm::com::BatchAck::abi_decode_params_validate(ack.as_ref())?;

                if instructions.len() != acknowledgements.len() {
                    Err(format!(
                        "invalid batch v0 shape, expected {} instructions but decoded {}",
                        instructions.len(),
                        acknowledgements.len()
                    ))?
                } else {
                    acknowledgements
                        .into_iter()
                        .zip(instructions)
                        .map(|(ack, shape)| BatchableInstructionV0Ack::decode(shape, ack))
                        .collect::<Result<Vec<_>>>()
                        .map(|instructions| BatchAck::V0(BatchV0Ack { instructions }))
                }
            }
        }
    }
}

// TODO: Non-empty
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchV0 {
    pub instructions: Vec<BatchableInstructionV0>,
}

// TODO: Non-empty
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchV0Ack {
    pub instructions: Vec<BatchableInstructionV0Ack>,
}

// TODO: Non-empty
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchV0Shape {
    pub instructions: Vec<BatchableInstructionV0Shape>,
}

impl BatchV0 {
    pub(crate) fn decode(operand: impl AsRef<[u8]>) -> Result<Self> {
        let ucs03_zkgm::com::Batch { instructions } =
            ucs03_zkgm::com::Batch::abi_decode_params_validate(operand.as_ref())?;
        Ok(Self {
            instructions: instructions
                .into_iter()
                .map(BatchableInstructionV0::from_raw)
                .collect::<Result<_>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum BatchableInstructionV0 {
    TokenOrder(TokenOrder),
    Call(Call),
    Stake(Stake),
    Unstake(Unstake),
    WithdrawStake(WithdrawStake),
    WithdrawRewards(WithdrawRewards),
}

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum BatchableInstructionV0Ack {
    TokenOrder(TokenOrderAck),
    Call(CallAck),
    // Stake(StakeAck),
    // Unstake(UnstakeAck),
    // WithdrawStake(WithdrawStakeAck),
    // WithdrawRewards(WithdrawRewardsAck),
}

impl BatchableInstructionV0Ack {
    fn decode(
        shape: BatchableInstructionV0Shape,
        ack: impl AsRef<[u8]>,
    ) -> Result<BatchableInstructionV0Ack> {
        match shape {
            BatchableInstructionV0Shape::TokenOrder(shape) => {
                TokenOrderAck::decode(shape, ack).map(BatchableInstructionV0Ack::TokenOrder)
            }
            BatchableInstructionV0Shape::Call(shape) => {
                CallAck::decode(shape, ack).map(BatchableInstructionV0Ack::Call)
            }
            BatchableInstructionV0Shape::Stake(_shape) => todo!(),
            BatchableInstructionV0Shape::Unstake(_shape) => todo!(),
            BatchableInstructionV0Shape::WithdrawStake(_shape) => {
                todo!()
            }
            BatchableInstructionV0Shape::WithdrawRewards(_shape) => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Enumorph)]
pub enum BatchableInstructionV0Shape {
    TokenOrder(TokenOrderShape),
    Call(CallShape),
    Stake(StakeShape),
    Unstake(UnstakeShape),
    WithdrawStake(WithdrawStakeShape),
    WithdrawRewards(WithdrawRewardsShape),
}

impl BatchableInstructionV0 {
    pub fn decode(bz: &[u8]) -> Result<Self> {
        let instruction = ucs03_zkgm::com::Instruction::abi_decode_params_validate(bz)?;

        Self::from_raw(instruction)
    }

    fn from_raw(instruction: ucs03_zkgm::com::Instruction) -> Result<BatchableInstructionV0> {
        match instruction.opcode {
            OP_TOKEN_ORDER => {
                TokenOrder::decode(instruction.version, instruction.operand).map(Into::into)
            }
            OP_CALL => Call::decode(instruction.version, instruction.operand).map(Into::into),
            OP_STAKE => Stake::decode(instruction.version, instruction.operand).map(Into::into),
            OP_UNSTAKE => Unstake::decode(instruction.version, instruction.operand).map(Into::into),
            OP_WITHDRAW_STAKE => {
                WithdrawStake::decode(instruction.version, instruction.operand).map(Into::into)
            }
            OP_WITHDRAW_REWARDS => {
                WithdrawRewards::decode(instruction.version, instruction.operand).map(Into::into)
            }
            invalid => Err(format!("invalid batch instruction opcode: {invalid}").into()),
        }
    }

    fn shape(&self) -> BatchableInstructionV0Shape {
        match self {
            BatchableInstructionV0::TokenOrder(token_order) => {
                BatchableInstructionV0Shape::TokenOrder(token_order.shape())
            }
            BatchableInstructionV0::Call(call) => BatchableInstructionV0Shape::Call(call.shape()),
            BatchableInstructionV0::Stake(stake) => {
                BatchableInstructionV0Shape::Stake(stake.shape())
            }
            BatchableInstructionV0::Unstake(unstake) => {
                BatchableInstructionV0Shape::Unstake(unstake.shape())
            }
            BatchableInstructionV0::WithdrawStake(withdraw_stake) => {
                BatchableInstructionV0Shape::WithdrawStake(withdraw_stake.shape())
            }
            BatchableInstructionV0::WithdrawRewards(withdraw_rewards) => {
                BatchableInstructionV0Shape::WithdrawRewards(withdraw_rewards.shape())
            }
        }
    }
}

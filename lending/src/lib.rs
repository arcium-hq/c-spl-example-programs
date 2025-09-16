#![allow(unexpected_cfgs)]

pub mod instruction;
pub mod processor;
pub mod state;

use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo, declare_id, entrypoint::ProgramResult, msg, pubkey::Pubkey,
};

use crate::{instruction::LendingInstruction, processor::*};

declare_id!("LEnd9tZRMSzvCktmhCeMEZXVMXLa2nEZ2QrCpMtr7dV");

solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match LendingInstruction::try_from_slice(instruction_data) {
        Ok(instruction) => match instruction {
            LendingInstruction::InitializeLendingPool {
                interest_rate_bps,
                loan_to_value_bps,
                collateral_threshold_bps,
            } => {
                msg!("InitializeLendingPool");
                process_initialize_lending_pool(
                    program_id,
                    accounts,
                    interest_rate_bps,
                    loan_to_value_bps,
                    collateral_threshold_bps,
                )
            }
            LendingInstruction::InitializeLoan => {
                msg!("InitializeLoan");
                process_initialize_loan(accounts)
            }
            LendingInstruction::Borrow {
                computation_offset,
                transfer_id,
            } => {
                msg!("Borrow");
                process_borrow(accounts, computation_offset, transfer_id)
            }
            LendingInstruction::BorrowCallback => {
                msg!("BorrowCallback");
                process_borrow_callback(accounts, instruction_data)
            }
            LendingInstruction::Repay {
                computation_offset,
                transfer_id,
            } => {
                msg!("Repay");
                process_repay(accounts, computation_offset, transfer_id)
            }
            LendingInstruction::RepayCallback => {
                msg!("RepayCallback");
                process_repay_callback(accounts, instruction_data)
            }
        },
        Err(e) => panic!("Failed to deserialize instruction {}", e),
    }
}

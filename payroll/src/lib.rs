#![allow(unexpected_cfgs)]

pub mod instruction;
pub mod processor;
pub mod state;

use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo, declare_id, entrypoint::ProgramResult, msg, pubkey::Pubkey,
};

use crate::{instruction::PayrollInstruction, processor::*};

declare_id!("PayYVqDEBoQ7BrAL3NzzEW5uTeZhGV9vdJpD25PQfnd");

solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match PayrollInstruction::try_from_slice(instruction_data) {
        Ok(instruction) => match instruction {
            PayrollInstruction::Initialize => {
                msg!("Initialize");
                process_initialize(program_id, accounts)
            }
            PayrollInstruction::AddEmployee {
                employee,
                encrypted_salary,
            } => {
                msg!("AddEmployee");
                process_add_employee(accounts, &employee, encrypted_salary)
            }
            PayrollInstruction::ClaimSalary {
                computation_offset,
                transfer_id,
            } => {
                msg!("ClaimSalary");
                process_claim_salary(accounts, computation_offset, transfer_id)
            }
            PayrollInstruction::ClaimSalaryCallback => {
                msg!("ClaimSalaryCallback");
                process_claim_salary_callback(accounts)
            }
        },
        Err(e) => panic!("Failed to deserialize instruction {}", e),
    }
}

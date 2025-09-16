use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::confidential_transfer_adapter::state::RescueCiphertext;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

pub const MAX_EMPLOYEES: usize = 8;

#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Copy, Clone)]
pub struct Employee {
    pub key: [u8; 32],
    pub encrypted_salary: RescueCiphertext,
    pub last_claimed_slot: u64,
    pub previous_claimed_slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Payroll {
    pub employer: [u8; 32],
    pub mint: [u8; 32],
    pub num_employees: u8,
    pub employees: [Employee; MAX_EMPLOYEES],
}

impl Payroll {
    pub fn new(employer: &Pubkey, mint: &Pubkey) -> Self {
        Self {
            employer: employer.to_bytes(),
            mint: mint.to_bytes(),
            ..Default::default()
        }
    }

    pub fn find_employee(&self, employee: &Pubkey) -> Result<usize, ProgramError> {
        let employee = employee.to_bytes();

        let mut found = false;
        let mut idx = 0;
        for i in 0..self.num_employees as usize {
            if self.employees[i].key == employee {
                found = true;
                idx = i;
                break;
            }
        }

        if !found {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(idx)
    }
}

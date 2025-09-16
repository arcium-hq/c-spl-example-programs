use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::confidential_transfer_adapter::state::RescueCiphertext;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use solana_program_error::ProgramResult;

pub const MAX_BORROWERS: usize = 8;

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct LendingPool {
    pub lender: [u8; 32],
    pub asset_mint: [u8; 32],
    pub collateral_mint: [u8; 32],

    pub interest_rate_bps: u16,
    pub loan_to_value_bps: u16,
    pub collateral_threshold_bps: u16,

    pub num_borrowers: u8,
    pub borrowers: [[u8; 32]; MAX_BORROWERS],
}

impl LendingPool {
    pub fn new(
        lender: &Pubkey,
        asset_mint: &Pubkey,
        collateral_mint: &Pubkey,
        interest_rate_bps: u16,
        loan_to_value_bps: u16,
        collateral_threshold_bps: u16,
    ) -> Self {
        Self {
            lender: lender.to_bytes(),
            asset_mint: asset_mint.to_bytes(),
            collateral_mint: collateral_mint.to_bytes(),
            interest_rate_bps,
            loan_to_value_bps,
            collateral_threshold_bps,
            ..Default::default()
        }
    }

    pub fn add_borrower(&mut self, borrower: &Pubkey) -> ProgramResult {
        if self.num_borrowers as usize >= MAX_BORROWERS {
            return Err(ProgramError::InvalidAccountData);
        }

        let borrower_idx = self.num_borrowers as usize;
        self.borrowers[borrower_idx] = borrower.to_bytes();
        self.num_borrowers += 1;

        Ok(())
    }

    pub fn find_borrower(&self, borrower: &Pubkey) -> Result<usize, ProgramError> {
        let borrower = borrower.to_bytes();

        let mut found = false;
        let mut idx = 0;
        for i in 0..self.num_borrowers as usize {
            if self.borrowers[i] == borrower {
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

#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Clone, Copy)]
pub struct Loan {
    pub borrower: [u8; 32],
    pub lending_pool: [u8; 32],
    pub active: bool,
    pub encrypted_principal: RescueCiphertext,
    pub encrypted_collateral: RescueCiphertext,
    pub last_update_slot: u64,
}

impl Loan {
    pub fn new(borrower: &Pubkey, lending_pool: &Pubkey) -> Self {
        Self {
            borrower: borrower.to_bytes(),
            lending_pool: lending_pool.to_bytes(),
            active: false,
            encrypted_principal: RescueCiphertext::default(),
            encrypted_collateral: RescueCiphertext::default(),
            last_update_slot: 0,
        }
    }
}

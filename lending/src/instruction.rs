use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::{
    confidential_spl_token_authority::derive_authority, get_arcium_processor_accounts,
    get_associated_confidential_token_account_address, get_associated_token_address_and_adapter,
    get_create_account_proof_context_state_address, get_key_registry_address,
    get_transfer_account_address, programs::system_program,
};
use solana_instruction::{AccountMeta, Instruction};
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

use crate::processor::{lending_pool_pda, loan_pda};

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize)]
pub enum LendingInstruction {
    InitializeLendingPool {
        interest_rate_bps: u16,
        loan_to_value_bps: u16,
        collateral_threshold_bps: u16,
    },

    InitializeLoan,

    Borrow {
        computation_offset: u32,
        transfer_id: u32,
    },
    BorrowCallback,

    Repay {
        computation_offset: u32,
        transfer_id: u32,
    },
    RepayCallback,
}

pub fn initialize_lending_pool(
    lender: &Pubkey,
    asset_mint: &Pubkey,
    collateral_mint: &Pubkey,
    interest_rate_bps: u16,
    loan_to_value_bps: u16,
    collateral_threshold_bps: u16,
) -> Result<Instruction, ProgramError> {
    let (lending_pool_pda, _) = lending_pool_pda(lender);
    let derived_lending_pool_authority = derive_authority(&lending_pool_pda).0;

    let (asset_vault_ata, asset_vault_ata_adapter) = get_associated_token_address_and_adapter(
        &lending_pool_pda,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let key_registry_info = get_key_registry_address(&crate::ID);
    let proof_context_state_info = get_create_account_proof_context_state_address(&crate::ID);

    let accounts = vec![
        AccountMeta::new(*lender, true),
        AccountMeta::new(lending_pool_pda, false),
        AccountMeta::new(derived_lending_pool_authority, false),
        AccountMeta::new_readonly(*asset_mint, false),
        AccountMeta::new_readonly(*collateral_mint, false),
        AccountMeta::new(asset_vault_ata, false),
        AccountMeta::new(asset_vault_ata_adapter, false),
        AccountMeta::new(proof_context_state_info, false),
        AccountMeta::new(key_registry_info, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token::ID,
            false,
        ),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_transfer_adapter::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token_authority::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::spl_associated_token_account::ID,
            false,
        ),
    ];
    let data = LendingInstruction::InitializeLendingPool {
        interest_rate_bps,
        loan_to_value_bps,
        collateral_threshold_bps,
    }
    .try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub fn initialize_loan(
    lender: &Pubkey,
    borrower: &Pubkey,
    asset_mint: &Pubkey,
    collateral_mint: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let lending_pool_pda = lending_pool_pda(lender).0;
    let loan_pda = loan_pda(lender, borrower).0;
    let derived_loan_authority = derive_authority(&loan_pda).0;

    let asset_vault_ata = get_associated_confidential_token_account_address(
        &lending_pool_pda,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );

    let (collateral_vault_ata, collateral_vault_ata_adapter) =
        get_associated_token_address_and_adapter(
            &loan_pda,
            collateral_mint,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            true,
        );
    let (asset_repay_ata, asset_repay_ata_adapter) = get_associated_token_address_and_adapter(
        &loan_pda,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );

    let key_registry_info = get_key_registry_address(&crate::ID);
    let proof_context_state_info = get_create_account_proof_context_state_address(&crate::ID);

    let accounts = vec![
        AccountMeta::new(*borrower, true),
        AccountMeta::new(*lender, false),
        AccountMeta::new(lending_pool_pda, false),
        AccountMeta::new(loan_pda, false),
        AccountMeta::new(derived_loan_authority, false),
        AccountMeta::new_readonly(*asset_mint, false),
        AccountMeta::new_readonly(*collateral_mint, false),
        AccountMeta::new_readonly(asset_vault_ata, false),
        AccountMeta::new(collateral_vault_ata, false),
        AccountMeta::new(collateral_vault_ata_adapter, false),
        AccountMeta::new(asset_repay_ata, false),
        AccountMeta::new(asset_repay_ata_adapter, false),
        AccountMeta::new(proof_context_state_info, false),
        AccountMeta::new(key_registry_info, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token::ID,
            false,
        ),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_transfer_adapter::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token_authority::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::spl_associated_token_account::ID,
            false,
        ),
    ];
    let data = LendingInstruction::InitializeLoan {}.try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub fn borrow(
    lender: &Pubkey,
    borrower: &Pubkey,
    asset_mint: &Pubkey,
    collateral_mint: &Pubkey,
    computation_offset: u32,
    transfer_id: u32,
) -> Result<Instruction, ProgramError> {
    let lending_pool_pda = lending_pool_pda(lender).0;
    let loan_pda = loan_pda(lender, borrower).0;
    let derived_lending_pool_authority = derive_authority(&lending_pool_pda).0;
    let derived_loan_authority = derive_authority(&loan_pda).0;

    // Vault ATAs.
    let (asset_vault_ata, asset_vault_ata_adapter) = get_associated_token_address_and_adapter(
        &lending_pool_pda,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let (collateral_vault_ata, collateral_vault_ata_adapter) =
        get_associated_token_address_and_adapter(
            &loan_pda,
            collateral_mint,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            true,
        );

    // Borrower ATAs.
    let asset_borrower_ata = get_associated_confidential_token_account_address(
        borrower,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        false,
    );
    let collateral_borrower_ata = get_associated_confidential_token_account_address(
        borrower,
        collateral_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        false,
    );

    let transfer_account =
        get_transfer_account_address(&[asset_vault_ata, collateral_vault_ata], transfer_id);
    let [mxe_account, computation_account] =
        get_arcium_processor_accounts(&crate::ID, computation_offset);

    let accounts = vec![
        AccountMeta::new(*borrower, true),
        AccountMeta::new(*lender, false),
        AccountMeta::new(lending_pool_pda, false),
        AccountMeta::new(derived_lending_pool_authority, false),
        AccountMeta::new(loan_pda, false),
        AccountMeta::new(derived_loan_authority, false),
        AccountMeta::new_readonly(*asset_mint, false),
        AccountMeta::new_readonly(*collateral_mint, false),
        // Source for asset transfer.
        AccountMeta::new(asset_vault_ata, false),
        AccountMeta::new(asset_vault_ata_adapter, false),
        // Source for excess collateral transfer.
        AccountMeta::new(collateral_vault_ata, false),
        AccountMeta::new(collateral_vault_ata_adapter, false),
        // Destination for asset transfer.
        AccountMeta::new_readonly(asset_borrower_ata, false),
        // Destination for excess collateral transfer.
        AccountMeta::new_readonly(collateral_borrower_ata, false),
        AccountMeta::new(transfer_account, false),
        AccountMeta::new(mxe_account, false),
        AccountMeta::new(computation_account, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token::ID,
            false,
        ),
        AccountMeta::new_readonly(confidential_spl_token::programs::arcium::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_transfer_adapter::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token_authority::ID,
            false,
        ),
    ];
    let data = LendingInstruction::Borrow {
        computation_offset,
        transfer_id,
    }
    .try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub(crate) fn borrow_callback(
    lender: &Pubkey,
    borrower: &Pubkey,
    transfer_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let (loan_pda, _) = loan_pda(lender, borrower);

    let accounts = vec![
        AccountMeta::new_readonly(loan_pda, false),
        AccountMeta::new_readonly(*transfer_account, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::instruction_sysvar::ID,
            false,
        ),
    ];
    let data = LendingInstruction::BorrowCallback.try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub fn repay(
    lender: &Pubkey,
    borrower: &Pubkey,
    asset_mint: &Pubkey,
    collateral_mint: &Pubkey,
    computation_offset: u32,
    transfer_id: u32,
) -> Result<Instruction, ProgramError> {
    let lending_pool_pda = lending_pool_pda(lender).0;
    let loan_pda = loan_pda(lender, borrower).0;
    // let derived_lending_pool_authority = derive_authority(&lending_pool_pda).0;
    let derived_loan_authority = derive_authority(&loan_pda).0;

    // Vault ATAs.
    let (asset_repay_ata, asset_repay_ata_adapter) = get_associated_token_address_and_adapter(
        &loan_pda,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let (collateral_vault_ata, collateral_vault_ata_adapter) =
        get_associated_token_address_and_adapter(
            &loan_pda,
            collateral_mint,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            true,
        );

    // Lender asset ATA.
    let asset_lender_ata = get_associated_confidential_token_account_address(
        lender,
        asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        false,
    );

    // Borrower collateral ATA.
    let collateral_borrower_ata = get_associated_confidential_token_account_address(
        borrower,
        collateral_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        false,
    );

    let transfer_account =
        get_transfer_account_address(&[asset_repay_ata, collateral_vault_ata], transfer_id);
    let [mxe_account, computation_account] =
        get_arcium_processor_accounts(&crate::ID, computation_offset);

    let accounts = vec![
        AccountMeta::new(*borrower, true),
        AccountMeta::new(*lender, false),
        AccountMeta::new(lending_pool_pda, false),
        AccountMeta::new(loan_pda, false),
        AccountMeta::new(derived_loan_authority, false),
        AccountMeta::new_readonly(*asset_mint, false),
        AccountMeta::new_readonly(*collateral_mint, false),
        // Source for asset transfer.
        AccountMeta::new(asset_repay_ata, false),
        AccountMeta::new(asset_repay_ata_adapter, false),
        // Source for excess collateral transfer.
        AccountMeta::new(collateral_vault_ata, false),
        AccountMeta::new(collateral_vault_ata_adapter, false),
        // Destination for asset transfer.
        AccountMeta::new_readonly(asset_lender_ata, false),
        // Destination for excess collateral transfer.
        AccountMeta::new_readonly(collateral_borrower_ata, false),
        AccountMeta::new(transfer_account, false),
        AccountMeta::new(mxe_account, false),
        AccountMeta::new(computation_account, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token::ID,
            false,
        ),
        AccountMeta::new_readonly(confidential_spl_token::programs::arcium::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_transfer_adapter::ID,
            false,
        ),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token_authority::ID,
            false,
        ),
    ];
    let data = LendingInstruction::Repay {
        computation_offset,
        transfer_id,
    }
    .try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub(crate) fn repay_callback(
    lender: &Pubkey,
    borrower: &Pubkey,
    transfer_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let (loan_pda, _) = loan_pda(lender, borrower);

    let accounts = vec![
        AccountMeta::new_readonly(loan_pda, false),
        AccountMeta::new_readonly(*transfer_account, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::instruction_sysvar::ID,
            false,
        ),
    ];
    let data = LendingInstruction::RepayCallback.try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

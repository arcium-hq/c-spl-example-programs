use crate::state::{LendingPool, Loan};
use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::confidential_spl_token_authority::Authority;
use confidential_spl_token::confidential_transfer_adapter::arcium_types::Argument;
use confidential_spl_token::confidential_transfer_adapter::state::RescueCiphertext;
use confidential_spl_token::invoke::TransferWithComputationInstruction;
use confidential_spl_token::{get_associated_token_address_and_adapter, transfer_result};
use solana_program::rent::Rent;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

pub(crate) fn process_initialize_lending_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    interest_rate_bps: u16,
    loan_to_value_bps: u16,
    collateral_threshold_bps: u16,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let lender_info = next_account_info(account_info_iter)?;
    let lending_pool_info = next_account_info(account_info_iter)?;
    let derive_lending_pool_authority_info = next_account_info(account_info_iter)?;
    let asset_mint_info = next_account_info(account_info_iter)?;
    let collateral_mint_info = next_account_info(account_info_iter)?;
    let asset_vault_ata_info = next_account_info(account_info_iter)?;
    let asset_vault_ata_adapter_info = next_account_info(account_info_iter)?;

    let proof_context_state_info = next_account_info(account_info_iter)?;
    let key_registry_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_program_info = next_account_info(account_info_iter)?;
    let ata_program_info = next_account_info(account_info_iter)?;

    if !lender_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (pda, bump) = check_lending_pool(
        lender_info.key,
        lending_pool_info,
        asset_mint_info,
        Some(asset_vault_ata_info),
        system_program_info.key,
    )?;

    // Create lending_pool_info.
    let lending_pool = LendingPool::new(
        lender_info.key,
        asset_mint_info.key,
        collateral_mint_info.key,
        interest_rate_bps,
        loan_to_value_bps,
        collateral_threshold_bps,
    );
    let lending_pool_data = lending_pool.try_to_vec()?;
    let lamports = Rent::get()?.minimum_balance(lending_pool_data.len());

    solana_cpi::invoke_signed(
        &solana_system_interface::instruction::create_account(
            lender_info.key,
            &pda,
            lamports,
            lending_pool_data.len() as u64,
            program_id,
        ),
        &[
            lender_info.clone(),
            lending_pool_info.clone(),
            system_program_info.clone(),
        ],
        &[&[b"lending_pool", lender_info.key.as_ref(), &[bump]]],
    )?;

    // Initialize lending_pool_info data.
    lending_pool_info
        .try_borrow_mut_data()?
        .copy_from_slice(&lending_pool_data);

    // We utilize a derived authority to have simpler callbacks.
    let authority = Authority::Derived {
        authority_info: &lending_pool_info.clone(),
        derived_authority_info: &derive_lending_pool_authority_info.clone(),
        confidential_spl_token_authority_program: &confidential_spl_token_authority_program_info
            .clone(),
    };

    // Create asset_vault_ata_info confidential SPL token account with lending_pool_info as the authority.
    confidential_spl_token::invoke::create_account(
        &crate::ID,
        lender_info,
        authority,
        asset_mint_info,
        asset_vault_ata_info,
        asset_vault_ata_adapter_info,
        system_program_info,
        token_program_info,
        ata_program_info,
        confidential_transfer_adapter_info,
        proof_context_state_info,
        key_registry_info,
        &[],
        &[&[b"lending_pool", lender_info.key.as_ref(), &[bump]]],
    )?;

    Ok(())
}

pub(crate) fn process_initialize_loan(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let borrower_info = next_account_info(account_info_iter)?;
    let lender_info = next_account_info(account_info_iter)?;
    let lending_pool_info = next_account_info(account_info_iter)?;
    let loan_info = next_account_info(account_info_iter)?;
    let derived_loan_info_authority = next_account_info(account_info_iter)?;

    let asset_mint_info = next_account_info(account_info_iter)?;
    let collateral_mint_info = next_account_info(account_info_iter)?;

    let asset_vault_ata_info = next_account_info(account_info_iter)?;

    let collateral_vault_ata_info = next_account_info(account_info_iter)?;
    let collateral_vault_ata_adapter_info = next_account_info(account_info_iter)?;
    let asset_repay_ata_info = next_account_info(account_info_iter)?;
    let asset_repay_ata_adapter_info = next_account_info(account_info_iter)?;

    let proof_context_state_info = next_account_info(account_info_iter)?;
    let key_registry_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_program_info = next_account_info(account_info_iter)?;
    let ata_program_info = next_account_info(account_info_iter)?;

    if !borrower_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    check_lending_pool(
        lender_info.key,
        lending_pool_info,
        asset_mint_info,
        Some(asset_vault_ata_info),
        &crate::ID,
    )?;

    let (loan_pda, bump) = check_loan(
        lender_info.key,
        borrower_info.key,
        loan_info,
        asset_mint_info,
        collateral_mint_info,
        collateral_vault_ata_info,
        Some(asset_repay_ata_info),
    )?;

    // Create loan_info account.
    let loan = Loan::new(borrower_info.key, lending_pool_info.key);
    let loan_data = loan.try_to_vec()?;
    let lamports = Rent::get()?.minimum_balance(loan_data.len());

    solana_cpi::invoke_signed(
        &solana_system_interface::instruction::create_account(
            borrower_info.key,
            &loan_pda,
            lamports,
            loan_data.len() as u64,
            &crate::ID,
        ),
        &[
            borrower_info.clone(),
            loan_info.clone(),
            system_program_info.clone(),
        ],
        &[&[
            b"loan",
            lender_info.key.as_ref(),
            borrower_info.key.as_ref(),
            &[bump],
        ]],
    )?;

    // Initialize loan_info data.
    loan_info.try_borrow_mut_data()?.copy_from_slice(&loan_data);

    // Add borrower to lending pool.
    let mut lending_pool = LendingPool::try_from_slice(&lending_pool_info.data.borrow())?;
    lending_pool.add_borrower(borrower_info.key)?;
    lending_pool_info
        .try_borrow_mut_data()?
        .copy_from_slice(&lending_pool.try_to_vec()?);

    // We utilize a derived authority to have simpler callbacks.
    let authority = Authority::Derived {
        authority_info: &loan_info.clone(),
        derived_authority_info: &derived_loan_info_authority.clone(),
        confidential_spl_token_authority_program: &confidential_spl_token_authority_program_info
            .clone(),
    };

    // Create collateral_vault_ata_info with loan_info as authority.
    confidential_spl_token::invoke::create_account(
        &crate::ID,
        borrower_info,
        authority.clone(),
        collateral_mint_info,
        collateral_vault_ata_info,
        collateral_vault_ata_adapter_info,
        system_program_info,
        token_program_info,
        ata_program_info,
        confidential_transfer_adapter_info,
        proof_context_state_info,
        key_registry_info,
        &[],
        &[&[
            b"loan",
            lender_info.key.as_ref(),
            borrower_info.key.as_ref(),
            &[bump],
        ]],
    )?;

    // Create asset_repay_ata_info with loan_info as authority.
    confidential_spl_token::invoke::create_account(
        &crate::ID,
        borrower_info,
        authority,
        asset_mint_info,
        asset_repay_ata_info,
        asset_repay_ata_adapter_info,
        system_program_info,
        token_program_info,
        ata_program_info,
        confidential_transfer_adapter_info,
        proof_context_state_info,
        key_registry_info,
        &[],
        &[&[
            b"loan",
            lender_info.key.as_ref(),
            borrower_info.key.as_ref(),
            &[bump],
        ]],
    )
}

pub const BORROW_COMP_DEF_OFFSET: u32 = 0;
pub const REPAY_COMP_DEF_OFFSET: u32 = 1;

pub(crate) fn process_borrow(
    accounts: &[AccountInfo],
    computation_offset: u32,
    transfer_id: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let borrower_info = next_account_info(account_info_iter)?;
    let lender_info = next_account_info(account_info_iter)?;
    let lending_pool_info = next_account_info(account_info_iter)?;
    let derived_lending_pool_authority_info = next_account_info(account_info_iter)?;
    let loan_info = next_account_info(account_info_iter)?;
    let derived_loan_authority_info = next_account_info(account_info_iter)?;
    let asset_mint_info = next_account_info(account_info_iter)?;
    let collateral_mint_info = next_account_info(account_info_iter)?;

    // Source for asset transfer.
    let asset_vault_ata_info = next_account_info(account_info_iter)?;
    let asset_vault_ata_adapter_info = next_account_info(account_info_iter)?;

    // Source for excess collateral transfer.
    let collateral_vault_ata_info: &AccountInfo<'_> = next_account_info(account_info_iter)?;
    let collateral_vault_ata_adapter_info = next_account_info(account_info_iter)?;

    // Destination for asset transfer.
    let asset_borrower_ata_info = next_account_info(account_info_iter)?;

    // Destination for excess collateral transfer.
    let collateral_borrower_ata_info = next_account_info(account_info_iter)?;

    let transfer_account_info = next_account_info(account_info_iter)?;
    let mxe_info = next_account_info(account_info_iter)?;
    let computation_info = next_account_info(account_info_iter)?;

    let system_program_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let arcium_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_program_info = next_account_info(account_info_iter)?;

    if !borrower_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (_, lending_pool_bump) = check_lending_pool(
        lender_info.key,
        lending_pool_info,
        asset_mint_info,
        Some(asset_vault_ata_info),
        &crate::ID,
    )?;

    let (_, loan_bump) = check_loan(
        lender_info.key,
        borrower_info.key,
        loan_info,
        asset_mint_info,
        collateral_mint_info,
        collateral_vault_ata_info,
        None,
    )?;

    // Transfer loan_amount to borrower.
    let asset_transfer = TransferWithComputationInstruction {
        authority: &Authority::Derived {
            authority_info: &lending_pool_info.clone(),
            derived_authority_info: &derived_lending_pool_authority_info.clone(),
            confidential_spl_token_authority_program:
                &confidential_spl_token_authority_program_info.clone(),
        },
        mint_info: asset_mint_info,
        source_token_account_info: asset_vault_ata_info,
        source_token_account_adapter_info: asset_vault_ata_adapter_info,
        destination_token_account_info: asset_borrower_ata_info,
        multisig_signers_infos: &[],
    };

    // Transfer collateral_excess_amount back to borrower.
    let collateral_transfer = TransferWithComputationInstruction {
        authority: &Authority::Derived {
            authority_info: &loan_info.clone(),
            derived_authority_info: &derived_loan_authority_info.clone(),
            confidential_spl_token_authority_program:
                &confidential_spl_token_authority_program_info.clone(),
        },
        mint_info: collateral_mint_info,
        source_token_account_info: collateral_vault_ata_info,
        source_token_account_adapter_info: collateral_vault_ata_adapter_info,
        destination_token_account_info: collateral_borrower_ata_info,
        multisig_signers_infos: &[],
    };

    // Arguments for the encrypted computation.
    let lending_pool = LendingPool::try_from_slice(&lending_pool_info.data.borrow())?;
    let price = 1;
    let arguments = [
        Argument::ConfidentialTokenAccount(asset_vault_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(asset_borrower_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(collateral_vault_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(collateral_borrower_ata_info.key.to_bytes()),
        Argument::PlaintextU16(price),
        Argument::PlaintextU16(lending_pool.loan_to_value_bps),
    ];

    // TODO: Freeze collateral vault.

    confidential_spl_token::invoke::transfer_with_computation(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &crate::ID,
        &[asset_transfer, collateral_transfer],
        &arguments,
        borrower_info,
        transfer_account_info,
        mxe_info,
        computation_info,
        system_program_info,
        token_program_info,
        arcium_program_info,
        confidential_transfer_adapter_info,
        crate::instruction::borrow_callback(
            lender_info.key,
            borrower_info.key,
            transfer_account_info.key,
        )?
        .into(),
        computation_offset,
        BORROW_COMP_DEF_OFFSET,
        transfer_id,
        &[
            &[
                b"lending_pool",
                lender_info.key.as_ref(),
                &[lending_pool_bump],
            ],
            &[
                b"loan",
                lender_info.key.as_ref(),
                borrower_info.key.as_ref(),
                &[loan_bump],
            ],
        ],
    )
}

pub(crate) fn process_borrow_callback(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let loan_info = next_account_info(account_info_iter)?;
    let transfer_account_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;

    let result = transfer_result(transfer_account_info, instructions_sysvar_info)?;

    // Take the custom output data from the computation.
    let output_data = result.custom_computation_output.unwrap();
    let encrypted_loan_amount = RescueCiphertext::try_from(&output_data[..])?;

    // Store the encrypted_loan_amount in the loan account.
    let mut loan = Loan::try_from_slice(&loan_info.try_borrow_data()?)?;
    loan.encrypted_principal = encrypted_loan_amount;
    loan_info
        .try_borrow_mut_data()?
        .copy_from_slice(&loan.try_to_vec()?);

    Ok(())
}

pub(crate) fn process_repay(
    accounts: &[AccountInfo],
    computation_offset: u32,
    transfer_id: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let borrower_info = next_account_info(account_info_iter)?;
    let lender_info = next_account_info(account_info_iter)?;
    let lending_pool_info = next_account_info(account_info_iter)?;
    let loan_info = next_account_info(account_info_iter)?;
    let derived_loan_authority_info = next_account_info(account_info_iter)?;
    let asset_mint_info = next_account_info(account_info_iter)?;
    let collateral_mint_info = next_account_info(account_info_iter)?;

    // Source for asset transfer.
    let asset_repay_ata_info = next_account_info(account_info_iter)?;
    let asset_repay_ata_adapter_info = next_account_info(account_info_iter)?;

    // Source for collateral transfer.
    let collateral_vault_ata_info: &AccountInfo<'_> = next_account_info(account_info_iter)?;
    let collateral_vault_ata_adapter_info = next_account_info(account_info_iter)?;

    // Destination for asset transfer.
    let asset_lender_ata_info = next_account_info(account_info_iter)?;

    // Destination for collateral transfer.
    let collateral_borrower_ata_info = next_account_info(account_info_iter)?;

    let transfer_account_info = next_account_info(account_info_iter)?;
    let mxe_info = next_account_info(account_info_iter)?;
    let computation_info = next_account_info(account_info_iter)?;

    let system_program_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let arcium_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_program_info = next_account_info(account_info_iter)?;

    if !borrower_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    check_lending_pool(
        lender_info.key,
        lending_pool_info,
        asset_mint_info,
        None,
        &crate::ID,
    )?;

    let (_, loan_bump) = check_loan(
        lender_info.key,
        borrower_info.key,
        loan_info,
        asset_mint_info,
        collateral_mint_info,
        collateral_vault_ata_info,
        Some(asset_repay_ata_info),
    )?;

    // Transfer actual_repay_amount from asset_repay_ata to lender.
    let asset_transfer = TransferWithComputationInstruction {
        authority: &Authority::Derived {
            authority_info: &loan_info.clone(),
            derived_authority_info: &derived_loan_authority_info.clone(),
            confidential_spl_token_authority_program:
                &confidential_spl_token_authority_program_info.clone(),
        },
        mint_info: asset_mint_info,
        source_token_account_info: asset_repay_ata_info,
        source_token_account_adapter_info: asset_repay_ata_adapter_info,
        destination_token_account_info: asset_lender_ata_info,
        multisig_signers_infos: &[],
    };

    // Transfer collateral_repayment from collateral_vault_ata to borrower.
    let collateral_transfer = TransferWithComputationInstruction {
        authority: &Authority::Derived {
            authority_info: &loan_info.clone(),
            derived_authority_info: &derived_loan_authority_info.clone(),
            confidential_spl_token_authority_program:
                &confidential_spl_token_authority_program_info.clone(),
        },
        mint_info: collateral_mint_info,
        source_token_account_info: collateral_vault_ata_info,
        source_token_account_adapter_info: collateral_vault_ata_adapter_info,
        destination_token_account_info: collateral_borrower_ata_info,
        multisig_signers_infos: &[],
    };

    // Arguments for the encrypted computation.
    let lending_pool = LendingPool::try_from_slice(&lending_pool_info.data.borrow())?;
    let loan = Loan::try_from_slice(&loan_info.data.borrow())?;
    let slots_elapsed = 10;
    let arguments = [
        Argument::ConfidentialTokenAccount(asset_repay_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(asset_lender_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(collateral_vault_ata_info.key.to_bytes()),
        Argument::ConfidentialTokenAccount(collateral_borrower_ata_info.key.to_bytes()),
        Argument::EncryptedU64(loan.encrypted_principal),
        Argument::PlaintextU64(slots_elapsed),
        Argument::PlaintextU16(lending_pool.interest_rate_bps),
    ];

    confidential_spl_token::invoke::transfer_with_computation(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &crate::ID,
        &[asset_transfer, collateral_transfer],
        &arguments,
        borrower_info,
        transfer_account_info,
        mxe_info,
        computation_info,
        system_program_info,
        token_program_info,
        arcium_program_info,
        confidential_transfer_adapter_info,
        crate::instruction::repay_callback(
            lender_info.key,
            borrower_info.key,
            transfer_account_info.key,
        )?
        .into(),
        computation_offset,
        REPAY_COMP_DEF_OFFSET,
        transfer_id,
        &[
            &[
                b"loan",
                lender_info.key.as_ref(),
                borrower_info.key.as_ref(),
                &[loan_bump],
            ],
            &[
                b"loan",
                lender_info.key.as_ref(),
                borrower_info.key.as_ref(),
                &[loan_bump],
            ],
        ],
    )
}

pub(crate) fn process_repay_callback(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let loan_info = next_account_info(account_info_iter)?;
    let transfer_account_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;

    let result = transfer_result(transfer_account_info, instructions_sysvar_info).unwrap();

    // Take the custom output data from the computation.
    let output_data = result.custom_computation_output.unwrap();
    let remaining_due = RescueCiphertext::try_from(&output_data[..32])?;
    let loan_is_fully_repaid = bool::try_from_slice(&output_data[32..])?;

    // Update the Loan account.
    let mut loan = Loan::try_from_slice(&loan_info.try_borrow_data()?)?;
    loan.encrypted_principal = remaining_due;
    loan.active = !loan_is_fully_repaid;
    loan_info
        .try_borrow_mut_data()?
        .copy_from_slice(&loan.try_to_vec()?);

    Ok(())
}

pub fn lending_pool_pda(lender: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lending_pool", lender.as_ref()], &crate::ID)
}

pub fn loan_pda(lender: &Pubkey, borrower: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"loan", lender.as_ref(), borrower.as_ref()], &crate::ID)
}

fn check_lending_pool(
    lender: &Pubkey,
    lending_pool_info: &AccountInfo,
    asset_mint_info: &AccountInfo,
    asset_vault_ata_info: Option<&AccountInfo>,
    lending_pool_owner: &Pubkey,
) -> Result<(Pubkey, u8), ProgramError> {
    let (pda, bump) = lending_pool_pda(lender);

    if lending_pool_info.key != &pda {
        return Err(ProgramError::InvalidAccountData);
    }

    if lending_pool_info.owner != lending_pool_owner {
        return Err(ProgramError::IncorrectProgramId);
    }

    let (expected_ata, _) = get_associated_token_address_and_adapter(
        &pda,
        asset_mint_info.key,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );

    if let Some(asset_vault_ata_info) = asset_vault_ata_info {
        if asset_vault_ata_info.key != &expected_ata {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    Ok((pda, bump))
}

fn check_loan(
    lender: &Pubkey,
    borrower: &Pubkey,
    loan_info: &AccountInfo,
    asset_mint_info: &AccountInfo,
    collateral_mint_info: &AccountInfo,
    collateral_vault_ata_info: &AccountInfo,
    asset_repay_ata_info: Option<&AccountInfo>,
) -> Result<(Pubkey, u8), ProgramError> {
    let (loan_pda, bump) = loan_pda(lender, borrower);

    if loan_info.key != &loan_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    let (collateral_vault_ata_pda, _) = get_associated_token_address_and_adapter(
        &loan_pda,
        collateral_mint_info.key,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    if collateral_vault_ata_info.key != &collateral_vault_ata_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    if let Some(asset_repay_ata_info) = asset_repay_ata_info {
        let (asset_repay_ata_pda, _) = get_associated_token_address_and_adapter(
            &loan_pda,
            asset_mint_info.key,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            true,
        );
        if asset_repay_ata_info.key != &asset_repay_ata_pda {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    Ok((loan_pda, bump))
}

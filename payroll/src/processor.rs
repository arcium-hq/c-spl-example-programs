use crate::state::{Employee, Payroll, MAX_EMPLOYEES};
use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::confidential_spl_token_authority::Authority;
use confidential_spl_token::confidential_transfer_adapter::state::{
    RescueCiphertext, TransferStatus,
};
use confidential_spl_token::{get_associated_confidential_token_account_address, transfer_result};
use solana_program::rent::Rent;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

pub(crate) fn process_initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let employer_info = next_account_info(account_info_iter)?;
    let payroll_info = next_account_info(account_info_iter)?;
    let derived_authority_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let payroll_token_account_info = next_account_info(account_info_iter)?;
    let payroll_token_account_adapter_info = next_account_info(account_info_iter)?;
    let proof_context_state_info = next_account_info(account_info_iter)?;
    let key_registry_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_info = next_account_info(account_info_iter)?;
    let ata_program_info = next_account_info(account_info_iter)?;

    if !employer_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // We utilize a derived authority to pass our signed invokations through.
    let authority = Authority::derived(
        payroll_info,
        derived_authority_info,
        confidential_spl_token_authority_info,
    );

    let (pda, bump) = check_payroll(
        employer_info,
        payroll_info,
        mint_info,
        payroll_token_account_info,
    )?;

    // Create payroll_info.
    let payroll = Payroll::new(employer_info.key, mint_info.key);
    let payroll_data = payroll.try_to_vec()?;
    let lamports = Rent::get()?.minimum_balance(payroll_data.len());

    solana_cpi::invoke_signed(
        &solana_system_interface::instruction::create_account(
            employer_info.key,
            &pda,
            lamports,
            payroll_data.len() as u64,
            program_id,
        ),
        &[
            employer_info.clone(),
            payroll_info.clone(),
            system_program_info.clone(),
        ],
        &[&[b"payroll", employer_info.key.as_ref(), &[bump]]],
    )?;

    // Initialize payroll_info data.
    payroll_info
        .try_borrow_mut_data()?
        .copy_from_slice(&payroll_data);

    // Create a confidential SPL token account with payroll_info as the authority.
    confidential_spl_token::invoke::create_account(
        &crate::ID,
        employer_info,
        authority,
        mint_info,
        payroll_token_account_info,
        payroll_token_account_adapter_info,
        system_program_info,
        token_program_info,
        ata_program_info,
        confidential_transfer_adapter_info,
        proof_context_state_info,
        key_registry_info,
        &[],
        &[&[b"payroll", employer_info.key.as_ref(), &[bump]]],
    )
}

pub(crate) fn process_add_employee(
    accounts: &[AccountInfo],
    employee: &[u8; 32],
    encrypted_salary: RescueCiphertext,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let employer_info = next_account_info(account_info_iter)?;
    let payroll_info = next_account_info(account_info_iter)?;

    if !employer_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Deserialize payroll.
    let mut payroll = Payroll::try_from_slice(&payroll_info.try_borrow_data()?)?;

    if payroll.employer != employer_info.key.to_bytes() {
        return Err(ProgramError::IllegalOwner);
    }

    if payroll.num_employees as usize >= MAX_EMPLOYEES {
        return Err(ProgramError::AccountDataTooSmall);
    }

    // Add new employee to payroll.
    payroll.employees[payroll.num_employees as usize] = Employee {
        key: *employee,
        encrypted_salary,
        last_claimed_slot: 0,
        previous_claimed_slot: 0,
    };
    payroll.num_employees += 1;

    // Write updates into payroll_info data.
    payroll_info
        .try_borrow_mut_data()?
        .copy_from_slice(&payroll.try_to_vec()?);

    Ok(())
}

pub(crate) fn process_claim_salary(
    accounts: &[AccountInfo],
    computation_offset: u32,
    transfer_id: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let employee_info = next_account_info(account_info_iter)?;
    let employee_token_account_info = next_account_info(account_info_iter)?;
    let employer_info = next_account_info(account_info_iter)?;
    let payroll_info = next_account_info(account_info_iter)?;
    let derived_authority_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let payroll_token_account_info = next_account_info(account_info_iter)?;
    let payroll_token_account_adapter_info = next_account_info(account_info_iter)?;
    let transfer_account_info = next_account_info(account_info_iter)?;
    let mxe_info = next_account_info(account_info_iter)?;
    let computation_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let arcium_program_info = next_account_info(account_info_iter)?;
    let confidential_transfer_adapter_info = next_account_info(account_info_iter)?;
    let confidential_spl_token_authority_info = next_account_info(account_info_iter)?;

    if !employee_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // We utilize a derived authority to pass our signed invokations through.
    let authority = Authority::derived(
        payroll_info,
        derived_authority_info,
        confidential_spl_token_authority_info,
    );

    let (_, bump) = check_payroll(
        employer_info,
        payroll_info,
        mint_info,
        payroll_token_account_info,
    )?;

    let mut payroll = Payroll::try_from_slice(&payroll_info.try_borrow_data()?)?;

    if mint_info.key.to_bytes() != payroll.mint {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // For simplicity, allow claim once per slot (could be per epoch, or time-based).
    let employee_idx = payroll.find_employee(employee_info.key)?;
    let clock = Clock::get()?;
    if payroll.employees[employee_idx].last_claimed_slot == clock.slot {
        msg!("Already claimed in this slot");
        return Err(ProgramError::Custom(0));
    }
    payroll.employees[employee_idx].previous_claimed_slot =
        payroll.employees[employee_idx].last_claimed_slot;
    payroll.employees[employee_idx].last_claimed_slot = clock.slot;

    payroll_info
        .try_borrow_mut_data()?
        .copy_from_slice(&payroll.try_to_vec()?);

    // claim_salary_callback should be called after the payroll transfer has been attemped.
    let callback_instruction = crate::instruction::claim_salary_callback(
        employee_token_account_info.key,
        employer_info.key,
        mint_info.key,
        transfer_id,
    )?
    .into();

    // The encrypted transfer amount is the employees encrypted salary.
    let encrypted_amount = payroll.employees[employee_idx].encrypted_salary.into();

    // Transfer salary from payroll_token_account_info to employee_token_account_info.
    confidential_spl_token::invoke::transfer(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &crate::ID,
        employee_info,
        authority,
        mint_info,
        payroll_token_account_info,
        payroll_token_account_adapter_info,
        employee_token_account_info,
        transfer_account_info,
        mxe_info,
        computation_info,
        system_program_info,
        token_program_info,
        arcium_program_info,
        confidential_transfer_adapter_info,
        &[],
        callback_instruction,
        encrypted_amount,
        computation_offset,
        transfer_id,
        &[&[b"payroll", employer_info.key.as_ref(), &[bump]]],
    )
}

pub(crate) fn process_claim_salary_callback(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let employer_info = next_account_info(account_info_iter)?;
    let payroll_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let payroll_token_account_info = next_account_info(account_info_iter)?;
    let _employee_token_account_info = next_account_info(account_info_iter)?;
    let transfer_account_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;

    check_payroll(
        employer_info,
        payroll_info,
        mint_info,
        payroll_token_account_info,
    )?;

    // Check if the transfer was successfull.
    let transfer_output = transfer_result(transfer_account_info, instructions_sysvar_info);
    if let Ok(output) = transfer_output {
        if output.status == TransferStatus::Success {
            return Ok(());
        }
    }

    // TODO: Since the transfer has failed, we allow the employee to claim their salary again.

    Ok(())
}

fn check_payroll(
    employer_info: &AccountInfo,
    payroll_info: &AccountInfo,
    mint_info: &AccountInfo,
    payroll_token_account_info: &AccountInfo,
) -> Result<(Pubkey, u8), ProgramError> {
    let (pda, bump) =
        Pubkey::find_program_address(&[b"payroll", employer_info.key.as_ref()], &crate::ID);

    if *payroll_info.key != pda {
        return Err(ProgramError::InvalidAccountOwner);
    }

    let ata = get_associated_confidential_token_account_address(
        payroll_info.key,
        mint_info.key,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );

    if *payroll_token_account_info.key != ata {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok((pda, bump))
}

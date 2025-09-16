use borsh::{BorshDeserialize, BorshSerialize};
use confidential_spl_token::{
    confidential_spl_token_authority::derive_authority,
    confidential_transfer_adapter::state::RescueCiphertext, get_adapter_address,
    get_arcium_processor_accounts, get_associated_confidential_token_account_address,
    get_create_account_proof_context_state_address, get_key_registry_address,
    get_single_transfer_account_address, programs::system_program,
};
use solana_instruction::{AccountMeta, Instruction};
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize)]
pub enum PayrollInstruction {
    Initialize,

    AddEmployee {
        employee: [u8; 32],
        encrypted_salary: RescueCiphertext,
    },

    ClaimSalary {
        computation_offset: u32,
        transfer_id: u32,
    },
    ClaimSalaryCallback,
}

pub fn initialize(employer: &Pubkey, mint: &Pubkey) -> Result<Instruction, ProgramError> {
    let (payroll_pda, _) =
        Pubkey::find_program_address(&[b"payroll", employer.as_ref()], &crate::ID);
    let derived_authority = derive_authority(&payroll_pda).0;
    let ata = get_associated_confidential_token_account_address(
        &payroll_pda,
        mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let adapter = get_adapter_address(&ata);
    let get_key_registry_address = get_key_registry_address(&crate::ID);
    let proof_context_state = get_create_account_proof_context_state_address(&crate::ID);

    let accounts = vec![
        AccountMeta::new(*employer, true),
        AccountMeta::new(payroll_pda, false), // authority
        AccountMeta::new_readonly(derived_authority, false), // derived authority
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(ata, false),
        AccountMeta::new(adapter, false),
        AccountMeta::new(proof_context_state, false),
        AccountMeta::new(get_key_registry_address, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::confidential_spl_token::ID,
            false,
        ),
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
    let data = PayrollInstruction::Initialize.try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub fn add_employee(
    employer: &Pubkey,
    employee: &Pubkey,
    encrypted_salary: RescueCiphertext,
) -> Result<Instruction, ProgramError> {
    let (payroll_pda, _) =
        Pubkey::find_program_address(&[b"payroll", employer.as_ref()], &crate::ID);

    let accounts = vec![
        AccountMeta::new(*employer, true),
        AccountMeta::new(payroll_pda, false),
        AccountMeta::new_readonly(*employee, false),
    ];
    let data = PayrollInstruction::AddEmployee {
        employee: employee.to_bytes(),
        encrypted_salary,
    }
    .try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

pub fn claim_salary(
    employee: &Pubkey,
    employee_token_account: &Pubkey,
    employer: &Pubkey,
    mint: &Pubkey,
    computation_offset: u32,
    transfer_id: u32,
) -> Result<Instruction, ProgramError> {
    let (payroll_pda, _) =
        Pubkey::find_program_address(&[b"payroll", employer.as_ref()], &crate::ID);
    let derived_authority = derive_authority(&payroll_pda).0;
    let ata = get_associated_confidential_token_account_address(
        &payroll_pda,
        mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let adapter = get_adapter_address(&ata);
    let transfer_account = get_single_transfer_account_address(&ata, transfer_id);
    let [mxe_account, computation_account] =
        get_arcium_processor_accounts(&crate::ID, computation_offset);

    let accounts = vec![
        AccountMeta::new(*employee, true),
        AccountMeta::new(*employee_token_account, false),
        AccountMeta::new(*employer, false),
        AccountMeta::new(payroll_pda, false),       // authority
        AccountMeta::new(derived_authority, false), // derived authority
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(ata, false),
        AccountMeta::new(adapter, false),
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
    let data = PayrollInstruction::ClaimSalary {
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

pub(crate) fn claim_salary_callback(
    employee_token_account: &Pubkey,
    employer: &Pubkey,
    mint: &Pubkey,
    transfer_id: u32,
) -> Result<Instruction, ProgramError> {
    let (payroll_pda, _) =
        Pubkey::find_program_address(&[b"payroll", employer.as_ref()], &crate::ID);
    let ata = get_associated_confidential_token_account_address(
        &payroll_pda,
        mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    let transfer_account = get_single_transfer_account_address(&ata, transfer_id);

    let accounts = vec![
        AccountMeta::new_readonly(*employer, false),
        AccountMeta::new(payroll_pda, false),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new_readonly(ata, false),
        AccountMeta::new_readonly(*employee_token_account, false),
        AccountMeta::new_readonly(transfer_account, false),
        AccountMeta::new_readonly(
            confidential_spl_token::programs::instruction_sysvar::ID,
            false,
        ),
    ];
    let data = PayrollInstruction::ClaimSalaryCallback.try_to_vec()?;

    Ok(Instruction {
        program_id: crate::ID,
        accounts,
        data,
    })
}

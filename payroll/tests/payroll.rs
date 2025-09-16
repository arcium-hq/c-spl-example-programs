use confidential_spl_token::get_associated_confidential_token_account_address;
use confidential_spl_token_test::{processor, tokio, ConfidentialSPLTokenTest, CustomProgram};
use solana_pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

#[tokio::test]
async fn test_payroll() {
    let mut test = ConfidentialSPLTokenTest::new(vec![CustomProgram {
        program_name: "payroll",
        program_id: payroll::ID,
        processor: processor!(payroll::process_instruction),
    }])
    .await;

    // Enable program to use confidential token accounts.
    let mxe_pubkey = test
        .enable_confidential_token_accounts_for_program(&payroll::ID)
        .await;

    // Setup Mint.
    let mint_authority = Keypair::new();
    let mint = test
        .create_mint(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            9,
            &mint_authority,
        )
        .await
        .pubkey();

    // Create employee.
    let employee = test.new_actor().await;
    employee
        .create_ata(
            &mut test,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &mint,
        )
        .await;

    // Create and fund employer.
    let employer = test.new_actor().await;
    employer
        .create_ata(
            &mut test,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &mint,
        )
        .await;
    test.mint_to_account(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &mint,
        &mint_authority,
        1000,
        &employer.ata(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &mint,
        ),
    )
    .await;
    employer.deposit(&mut test, &mint, 1000).await;
    employer.apply_pending_balance(&mut test, &mint).await;

    assert_eq!(0, employer.pending_balance(&mut test, &mint).await.unwrap());
    assert_eq!(
        1000,
        employer.available_balance(&mut test, &mint).await.unwrap()
    );

    // Create payroll account with associated confidetial SPL token account.
    let initialize_instruction =
        payroll::instruction::initialize(&employer.pubkey(), &mint).unwrap();
    let initialize_tx = Transaction::new_signed_with_payer(
        &[initialize_instruction],
        Some(&employer.pubkey()),
        &[&employer.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(initialize_tx, true).await.unwrap();

    // Employer transfers into the confidetial SPL token account.
    let (payroll, _) =
        Pubkey::find_program_address(&[b"payroll", employer.pubkey().as_ref()], &payroll::ID);
    let payroll_token_account = get_associated_confidential_token_account_address(
        &payroll,
        &mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    employer
        .transfer(&mut test, &mint, 1000, &payroll_token_account)
        .await;

    assert_eq!(
        1000,
        test.pending_balance(&payroll_token_account, &mxe_pubkey)
            .await
            .unwrap()
    );
    assert_eq!(
        0,
        test.available_balance(&payroll_token_account, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Add employee.
    let salary = 100;
    let encrypted_salary = test.get_mxe(&mxe_pubkey).unwrap().rescue_encrypt(salary);
    let add_employee_tx = Transaction::new_signed_with_payer(
        &[payroll::instruction::add_employee(
            &employer.pubkey(),
            &employee.pubkey(),
            encrypted_salary,
        )
        .unwrap()],
        Some(&employer.pubkey()),
        &[&employer.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(add_employee_tx, false)
        .await
        .unwrap();

    // Transfer salary to employee.
    let transfer_id = 0;
    let claim_salary_tx = Transaction::new_signed_with_payer(
        &[payroll::instruction::claim_salary(
            &employee.pubkey(),
            &employee.ata(
                &confidential_spl_token::programs::confidential_spl_token::ID,
                &mint,
            ),
            &employer.pubkey(),
            &mint,
            1,
            transfer_id,
        )
        .unwrap()],
        Some(&employee.pubkey()),
        &[&employee.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(claim_salary_tx, false)
        .await
        .unwrap();

    // Verify that employee has received the salary.
    assert_eq!(
        salary,
        employee.pending_balance(&mut test, &mint).await.unwrap()
    );
    assert_eq!(
        0,
        employee.available_balance(&mut test, &mint).await.unwrap()
    );

    // Verify that the program has been deducted the salary amount.
    assert_eq!(
        1000 - salary,
        test.available_balance(&payroll_token_account, &mxe_pubkey)
            .await
            .unwrap()
    );
    assert_eq!(
        0,
        test.pending_balance(&payroll_token_account, &mxe_pubkey)
            .await
            .unwrap()
    );
}

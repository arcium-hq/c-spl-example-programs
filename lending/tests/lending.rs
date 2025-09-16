use std::cmp::min;

use borsh::BorshDeserialize;
use confidential_spl_token::get_associated_confidential_token_account_address;
use confidential_spl_token_test::{processor, tokio, ConfidentialSPLTokenTest, CustomProgram};
use lending::{
    processor::{lending_pool_pda, loan_pda, BORROW_COMP_DEF_OFFSET, REPAY_COMP_DEF_OFFSET},
    state::Loan,
};
use solana_pubkey::Pubkey;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

#[tokio::test]
async fn test_lending() {
    // Setup test with the lending program.
    let mut test = ConfidentialSPLTokenTest::new(vec![CustomProgram {
        program_name: "lending",
        program_id: lending::ID,
        processor: processor!(lending::process_instruction),
    }])
    .await;

    // Enable program to use confidential token accounts.
    let mxe_pubkey = test
        .enable_confidential_token_accounts_for_program(&lending::ID)
        .await;

    // Setup borrow computation definition account.
    let compiled_borrow_circuit = lending_encrypted_ixs::encrypted_computations::borrow();
    test.create_comp_def_for_test(
        &lending::ID,
        BORROW_COMP_DEF_OFFSET,
        compiled_borrow_circuit,
    )
    .await
    .unwrap();

    // Setup repay computation definition account.
    let compiled_repay_circuit = lending_encrypted_ixs::encrypted_computations::repay();
    test.create_comp_def_for_test(&lending::ID, REPAY_COMP_DEF_OFFSET, compiled_repay_circuit)
        .await
        .unwrap();

    // Setup Mints.
    let asset_mint_authority = Keypair::new();
    let asset_mint = test
        .create_mint(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            9,
            &asset_mint_authority,
        )
        .await
        .pubkey();

    let collateral_mint_authority = Keypair::new();
    let collateral_mint: Pubkey = test
        .create_mint(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            9,
            &collateral_mint_authority,
        )
        .await
        .pubkey();

    // Setup lender.
    let lender = test.new_actor().await;
    lender
        .create_ata(
            &mut test,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &asset_mint,
        )
        .await;

    // Fund lender with assets.
    let asset_amount = 1000;
    test.mint_to_account(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &asset_mint,
        &asset_mint_authority,
        asset_amount,
        &lender.ata(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &asset_mint,
        ),
    )
    .await;
    lender.deposit(&mut test, &asset_mint, asset_amount).await;
    lender.apply_pending_balance(&mut test, &asset_mint).await;
    assert_eq!(
        asset_amount,
        lender
            .available_balance(&mut test, &asset_mint)
            .await
            .unwrap()
    );

    // Initialize lending pool.
    let interest_rate_bps = 1;
    let loan_to_value_bps = 10_000;
    let collateral_threshold_bps = 1;
    let init_lending_pool_tx = Transaction::new_signed_with_payer(
        &[lending::instruction::initialize_lending_pool(
            &lender.pubkey(),
            &asset_mint,
            &collateral_mint,
            interest_rate_bps,
            loan_to_value_bps,
            collateral_threshold_bps,
        )
        .unwrap()],
        Some(&test.get_payer().pubkey()),
        &[&test.get_payer(), &lender.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(init_lending_pool_tx, false)
        .await
        .unwrap();

    // Setup borrower.
    let borrower = test.new_actor().await;
    borrower
        .create_ata(
            &mut test,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &asset_mint,
        )
        .await;
    borrower
        .create_ata(
            &mut test,
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &collateral_mint,
        )
        .await;

    // Fund borrower with collateral.
    let collateral_amount = 2500;
    test.mint_to_account(
        &confidential_spl_token::programs::confidential_spl_token::ID,
        &collateral_mint,
        &collateral_mint_authority,
        collateral_amount,
        &borrower.ata(
            &confidential_spl_token::programs::confidential_spl_token::ID,
            &collateral_mint,
        ),
    )
    .await;
    borrower
        .deposit(&mut test, &collateral_mint, collateral_amount)
        .await;
    borrower
        .apply_pending_balance(&mut test, &collateral_mint)
        .await;
    assert_eq!(
        collateral_amount,
        borrower
            .available_balance(&mut test, &collateral_mint)
            .await
            .unwrap()
    );

    // Initialize loan.
    let init_loan_tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_price(1),
            ComputeBudgetInstruction::set_compute_unit_limit(400_000),
            lending::instruction::initialize_loan(
                &lender.pubkey(),
                &borrower.pubkey(),
                &asset_mint,
                &collateral_mint,
            )
            .unwrap(),
        ],
        Some(&test.get_payer().pubkey()),
        &[&test.get_payer(), &borrower.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(init_loan_tx, false).await.unwrap();

    // Borrower deposits all their collateral into the collateral_vault_ata.
    let loan_account = loan_pda(&lender.pubkey(), &borrower.pubkey()).0;
    let collateral_vault_ata = get_associated_confidential_token_account_address(
        &loan_account,
        &collateral_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    borrower
        .transfer(
            &mut test,
            &collateral_mint,
            collateral_amount,
            &collateral_vault_ata,
        )
        .await;

    // Check that borrower has deposited all of their tokens into the collateral_vault_ata.
    assert_eq!(
        0,
        borrower
            .total_balance(&mut test, &collateral_mint)
            .await
            .unwrap()
    );
    assert_eq!(
        collateral_amount,
        test.total_balance(&collateral_vault_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Lender provides liquidity into the asset_vault_ata.
    let lending_pool = lending_pool_pda(&lender.pubkey()).0;
    let asset_vault_ata = get_associated_confidential_token_account_address(
        &lending_pool,
        &asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    lender
        .transfer(&mut test, &asset_mint, asset_amount, &asset_vault_ata)
        .await;

    // Check that lender has deposited all of their tokens into the pool.
    assert_eq!(
        0,
        lender
            .available_balance(&mut test, &asset_mint)
            .await
            .unwrap()
    );
    assert_eq!(
        asset_amount,
        test.pending_balance(&asset_vault_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Borrower borrows tokens.
    let borrow_transfer_id = 0;
    let borrow_tx = Transaction::new_signed_with_payer(
        &[lending::instruction::borrow(
            &lender.pubkey(),
            &borrower.pubkey(),
            &asset_mint,
            &collateral_mint,
            1,
            borrow_transfer_id,
        )
        .unwrap()],
        Some(&test.get_payer().pubkey()),
        &[&test.get_payer(), &borrower.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(borrow_tx, false).await.unwrap();

    // Compute the expected values that should have been computed correctly in the MXE based on the encrypted balances.
    let price = 1u64;
    let max_loan_amount = mul_base_points(collateral_amount, price * loan_to_value_bps as u64);
    let loan_amount = min(max_loan_amount, asset_amount);
    let loan_collateral_amount = div_base_points(loan_amount, price * loan_to_value_bps as u64);
    let collateral_excess_amount = collateral_amount - loan_collateral_amount;

    // The borrower should have received loan_amount of asset.
    assert_eq!(
        loan_amount,
        borrower
            .total_balance(&mut test, &asset_mint)
            .await
            .unwrap()
    );

    // The borrower should have recieved (back) collateral_excess_amount of collateral.
    assert_eq!(
        collateral_excess_amount,
        borrower
            .total_balance(&mut test, &collateral_mint)
            .await
            .unwrap()
    );

    // The asset_vault_ata should now have asset_amount - loan_amount.
    assert_eq!(
        asset_amount - loan_amount,
        test.pending_balance(&asset_vault_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // The collateral_vault_ata should now have loan_collateral_amount.
    assert_eq!(
        loan_collateral_amount,
        test.total_balance(&collateral_vault_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Check updated state in loan account.
    let loan = Loan::try_from_slice(&test.get_account(&loan_account).await.unwrap().data).unwrap();
    assert_eq!(
        test.get_mxe(&mxe_pubkey)
            .unwrap()
            .rescue_decrypt(loan.encrypted_principal),
        loan_amount
    );

    // TODO: Simulate slots elapsing to accrue interest.
    let slots_elapsed = 10;

    // Borrower (partially) repays loan.
    let repay_amount = 100;
    let asset_repay_ata = get_associated_confidential_token_account_address(
        &loan_account,
        &asset_mint,
        &confidential_spl_token::programs::confidential_spl_token::ID,
        true,
    );
    borrower.apply_pending_balance(&mut test, &asset_mint).await;
    borrower
        .transfer(&mut test, &asset_mint, repay_amount, &asset_repay_ata)
        .await;

    // The asset_repay_ata should now have repay_amount.
    assert_eq!(
        repay_amount,
        test.pending_balance(&asset_repay_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Borrower calls repay instruction to receive collateral.
    let repay_transfer_id = 1;
    let repay_tx = Transaction::new_signed_with_payer(
        &[lending::instruction::repay(
            &lender.pubkey(),
            &borrower.pubkey(),
            &asset_mint,
            &collateral_mint,
            2,
            repay_transfer_id,
        )
        .unwrap()],
        Some(&test.get_payer().pubkey()),
        &[&test.get_payer(), &borrower.signer_keypair()],
        test.get_recent_blockhash(),
    );
    test.process_transaction(repay_tx, false).await.unwrap();

    let remaining_principal = loan_amount;
    let locked_collateral = loan_collateral_amount;

    let interest_accrued = mul_base_points(
        remaining_principal,
        interest_rate_bps as u64 * slots_elapsed,
    );
    let total_due = remaining_principal + interest_accrued;
    let actual_repay_amount = min(repay_amount, total_due);
    let remaining_due = total_due - actual_repay_amount;
    let collateral_repayment = (actual_repay_amount / total_due) * locked_collateral;
    let loan_is_fully_repaid = remaining_due.eq(&0);

    let loan = Loan::try_from_slice(&test.get_account(&loan_account).await.unwrap().data).unwrap();

    // Check that the Loan account contains the correct (public and encrypted) computation outputs.
    assert_eq!(loan.active, !loan_is_fully_repaid);
    assert_eq!(
        test.get_mxe(&mxe_pubkey)
            .unwrap()
            .rescue_decrypt(loan.encrypted_principal),
        remaining_due
    );

    // Check that borrower has received collateral_repayment in collateral (previousl balance: collateral_excess_amount).
    assert_eq!(
        collateral_repayment + collateral_excess_amount,
        borrower
            .pending_balance(&mut test, &collateral_mint)
            .await
            .unwrap()
    );
    // Check that collateral_vault_ata has transfered collateral_repayment.
    assert_eq!(
        loan_collateral_amount - collateral_repayment,
        test.total_balance(&collateral_vault_ata, &mxe_pubkey)
            .await
            .unwrap()
    );

    // Check that lender has received actual_repay_amount in asset.
    assert_eq!(
        actual_repay_amount,
        lender
            .pending_balance(&mut test, &asset_mint)
            .await
            .unwrap()
    );
    // Check that repay_ata is empty.
    assert_eq!(
        0,
        test.pending_balance(&asset_repay_ata, &mxe_pubkey)
            .await
            .unwrap()
    );
}

fn mul_base_points(a: u64, bps: u64) -> u64 {
    a * bps / 10_000
}

fn div_base_points(a: u64, bps: u64) -> u64 {
    a * 10_000 / bps
}

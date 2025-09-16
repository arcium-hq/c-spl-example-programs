use arcis::imports::*;
use confidential_spl_token::{ConfidentialTokenAccount, ConfidentialTransfer};

#[encrypted]
pub mod encrypted_computations {
    use super::*;

    #[instruction]
    pub fn borrow(
        mxe: Mxe,
        asset_vault_ata: ConfidentialTokenAccount,
        asset_borrower_ata: ConfidentialTokenAccount,
        collateral_vault_ata: ConfidentialTokenAccount,
        collateral_borrower_ata: ConfidentialTokenAccount,
        price: u64,
        loan_to_value_bps: u16,
    ) -> (ConfidentialTransfer, ConfidentialTransfer, Enc<Mxe, u64>) {
        let asset_amount = asset_vault_ata.encrypted_balance();
        let collateral_amount = collateral_vault_ata.encrypted_balance();

        let loan_to_value_bps_ratio = BasePoints(price * loan_to_value_bps);
        let max_loan_amount = collateral_amount.clone() * loan_to_value_bps_ratio.clone();
        let loan_amount = min(max_loan_amount, asset_amount);
        let loan_collateral_amount = loan_amount.clone() / loan_to_value_bps_ratio;
        let collateral_excess_amount = collateral_amount - loan_collateral_amount;

        // Transfer of loan_amount to the borrower.
        let asset_transfer = confidential_spl_token::transfer(
            &mxe,
            &asset_vault_ata,
            &asset_borrower_ata,
            loan_amount.clone(),
        );

        // Transfer of collateral_excess_amount to the borrower.
        let collateral_transfer = confidential_spl_token::transfer(
            &mxe,
            &collateral_vault_ata,
            &collateral_borrower_ata,
            collateral_excess_amount,
        );

        (
            asset_transfer,
            collateral_transfer,
            mxe.from_arcis(loan_amount),
        )
    }

    #[instruction]
    #[allow(clippy::too_many_arguments)]
    pub fn repay(
        mxe: Mxe,
        asset_repay_ata: ConfidentialTokenAccount,
        asset_lender_ata: ConfidentialTokenAccount,
        collateral_vault_ata: ConfidentialTokenAccount,
        collateral_borrower_ata: ConfidentialTokenAccount,
        remaining_principal: Enc<Mxe, u64>,
        slots_elapsed: u64,
        interest_rate_bps: u16,
    ) -> (
        ConfidentialTransfer,
        ConfidentialTransfer,
        Enc<Mxe, u64>,
        bool,
    ) {
        // Confidential token account balances.
        let repay_amount = asset_repay_ata.encrypted_balance();
        let locked_collateral = collateral_vault_ata.encrypted_balance();

        let remaining_principal = remaining_principal.to_arcis();
        let interest_accrued =
            remaining_principal.clone() * BasePoints(interest_rate_bps * slots_elapsed);
        let total_due = remaining_principal + interest_accrued;
        let actual_repay_amount = min(repay_amount, total_due.clone());
        let remaining_due = total_due.clone() - actual_repay_amount.clone();
        let collateral_repayment = (actual_repay_amount.clone() / total_due) * locked_collateral;
        let loan_is_fully_repaid = remaining_due.eq(0);

        // Transfer of actual_repay_amount to the lender.
        let asset_transfer = confidential_spl_token::transfer(
            &mxe,
            &asset_repay_ata,
            &asset_lender_ata,
            actual_repay_amount,
        );

        // Transfer of collateral_repayment to the borrower.
        let collateral_transfer = confidential_spl_token::transfer(
            &mxe,
            &collateral_vault_ata,
            &collateral_borrower_ata,
            collateral_repayment,
        );

        (
            asset_transfer,
            collateral_transfer,
            mxe.from_arcis(remaining_due),
            loan_is_fully_repaid.reveal(),
        )
    }
}

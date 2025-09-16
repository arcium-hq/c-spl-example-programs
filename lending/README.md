# Confidential Lending Program Example

This is an example confidential lending program using Arcium's `confidential-spl-token`.

The program supports:
- Confidential lending and borrowing for any SPL token pair,
- Configurable interest rates, loan-to-value ratios, and collateral thresholds,
- Full confidentiality of collateral and loan amounts, even during liquidation,
- Handles confidential repayment for overpayment of collateral and principal,
- Seamless client interaction: Users do not need to use `confidential-spl-token` themselves — they can interact with the program using standard `spl-token-2022` accounts, while the program manages confidential token flows internally using `confidential-spl-token`.

All of this is achieved without custom MPC or encrypted computation, leveraging only the native confidentiality features of `confidential-spl-token`.

## Actors
- `lender`: lends token against collateral and receives interest for it.
- `borrower`: borrows token against collateral and pays interest for it.
- `liquidator`: TODO

## Token Mints
- `asset_mint` (token to be lent and borrowed)
- `collateral_mint` (token to be used as collateral against the asset)

## Accounts
A `lender` can open a `LendingPool` account for a pair of `asset_mint` & `collateral_mint`:
```rust
struct LendingPool {
    lender: Pubkey,
    asset_mint: Pubkey,
    collateral_mint: Pubkey,
    interest_rate_bps: u16,
    loan_to_value_bps: u16,
    collateral_threshold_bps: u16,
}
```
Each `LendingPool` account has one confidential token account associated:
- `asset_vault_ata`: stores the total number of assets that can be borrowed, lender can freely deposit and withdraw from this vault

A `borrower` can open a `Loan` account for a `LendingPool`:
```rust
struct Loan {
    borrower: Pubkey,
    lending_pool: Pubkey,
    encrypted_principal: EncryptedAmount,
    last_update_slot: u64,
}
```

The `Loan` account has two confidential token accounts associated:
- `collateral_vault_ata`: stores the collateral for the loan
- `asset_repay_ata`: stores the asset to be repaid by the borrower

## Formulas

Let:

- `collateral_amount` be the number of `collateral_mint` tokens deposited in the loan,
- `loan_amount` be the amount of `asset_mint` borrowed,
- `loan_to_value_bps` be the maximum allowed loan-to-value ratio in basis points,
- `collateral_threshold_bps` be the liquidation threshold in basis points,
- `interest_rate_bps` be the annual interest rate in basis points,
- `price` be the value of 1 unit of `collateral_mint` in units of `asset_mint`.

Let:

- `max_loan_amount` = ((`collateral_amount` × `price`) × `loan_to_value_bps`) / 10_000,
- `collateral_amount` = (max_loan_amount * 10_000) / (`price` × `loan_to_value_bps`)
- `health_factor` = (`collateral_amount` × `price` × `collateral_threshold_bps`) / (`loan_amount` × 10_000).

## Flow

### Lending Pool Initialization
- `lender` calls `initialize_lending_pool`:
    - opens a `LendingPool` account
    - initializes `asset_vault_ata` (confidential token account)
- `lender` deposits `asset_mint` tokens into `asset_vault_ata`
    - can withdraw freely as long as liquidity isn't tied up in loans

### Borrowing
- `borrower` calls `initialize_loan`:
    - creates a `Loan` account
    - initializes `collateral_vault_ata` (confidential token account)
    - initializes `asset_repay_ata` (confidential token account)
- `borrower` deposits `collateral_mint` tokens into `collateral_vault_ata`
    - until they start borrowing, they can freely deposit/withdraw
- `borrower` calls `borrow`:
    - takes the encrypted balance of `collateral_vault_ata` into `encrypted_collateral_amount`
    - computes (defines formulas and constants on-chain, executed in full confidentiality within MPC):
        - `max_loan_amount`
        - `loan_amount` = min(max_loan_amount, available_in_asset_vault)
        - `loan_collateral_amount` = loan_amount / price × 10_000 / loan_to_value_bps
        - `collateral_excess_amount` = collateral_amount - loan_collateral_amount
    - transfers `collateral_excess_amount` back to the `borrower`
    - locks the `collateral_vault_ata` (no more deposits and withdrawals possible)
    - transfers `loan_amount` of `asset_mint` from `asset_vault_ata` to the `borrower`

### Repayment
- `borrower` transfers the confidential `repay_amount` of `asset_mint` into `asset_repay_ata`
- `borrower` calls `repay`:
    - the protocol calculates (confidentialy):
        - `slots_elapsed` = current_slot - last_update_slot
        - `interest_accrued` = remaining_principal * interest_rate_per_slot * slots_elapsed
        - `total_due` = remaining_principal + interest_accrued
        - `actual_repay_amount` = min(repay_amount, total_due)
        - `overpayment` = repay_amount - actual_repay_amount
        - `remaining_due` = total_due - actual_repay_amount
        - `collateral_repayment` = (actual_repay_amount / total_due) × locked_collateral
    - sets in `Loan` account:
        - `remaining_principal` := remaining_due
        - `last_update_slot` := current_slot
    - transfers `actual_repay_amount` from `asset_repay_ata` to the `lender`
    - transfers `collateral_repayment` from `collateral_vault_ata` back to the `borrower`

### Loan Closing
- if the loan has been fully repaid or has been fully liquidated, the loan can be closed
- `borrower` calls `close_loan`:
    - if `remaining_due = 0`:
        - closes accounts:
            - closes the `Loan` account
            - closes `collateral_vault_ata`
            - closes `asset_repay_ata` (borrower can claim overpayment amount)
            - rent is paid back to `borrower`

### Liquidation
- liquidation can only occur if: `health_factor < 1`
- any third-party `liquidator` can repay part or all of the `borrower`'s loan:
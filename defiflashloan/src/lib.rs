use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer, TokenAccount, Token, Mint};
use anchor_lang::solana_program::{clock::Clock, instruction::{Instruction, AccountMeta}, program::invoke};


declare_id!("9o3VbMAbvmXmrj4QJ35voJ3ScpccEATRAsi2zuFyUj2a");

const FEE_BPS: u64 = 50; // Default fee is 0.5%
const MAX_LOAN_AMOUNT: u64 = 1_000_000; // Maximum loan amount allowed
const LOAN_COOLDOWN: i64 = 60; // Cooldown between loans in seconds
const GRACE_PERIOD: i64 = 30; // Grace period for repayment in seconds

#[program]
pub mod flash_loan {
    use super::*;

    pub fn execute_flash_loan(
        ctx: Context<ExecuteFlashLoan>,
        loan_amount: u64,
        loan_expiration: i64,
    ) -> Result<()> {
        let loan = &ctx.accounts.loan_vault;
        let borrower = &ctx.accounts.borrower_account;

        // Ensure loan does not exceed maximum allowed amount
        require!(loan_amount <= MAX_LOAN_AMOUNT, FlashLoanError::LoanAmountTooLarge);

        // Ensure the loan vault has enough liquidity
        require!(loan.amount >= loan_amount, FlashLoanError::InsufficientFunds);

        // Ensure the loan has not expired (with grace period)
        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp <= loan_expiration + GRACE_PERIOD,
            FlashLoanError::LoanExpired
        );

        // Cooldown check
        require!(
            clock.unix_timestamp >= ctx.accounts.loan_state.last_loan_timestamp + LOAN_COOLDOWN,
            FlashLoanError::CooldownPeriodNotOver
        );

        // Reentrancy check
        require!(!ctx.accounts.loan_state.active, FlashLoanError::Reentrancy);
        ctx.accounts.loan_state.active = true;

        //  Transfer loan amount to borrower
        token::transfer(
            ctx.accounts.into_transfer_to_borrower_context(),
            loan_amount,
        )?;

        //  Execute a Cross-Program Invocation (CPI)
        // Assuming you're invoking some external program (e.g., a token swap)
        // Construct the instruction
        let ix = Instruction {
            program_id: ctx.accounts.token_program.key(), // Replace with the actual program ID you are calling
            accounts: vec![
                AccountMeta::new(ctx.accounts.loan_vault.key(), false),  // Loan vault
                AccountMeta::new(ctx.accounts.borrower_account.key(), false), // Borrower account
                // Add other accounts required by the external program
            ],
            data: vec![], // Add the actual instruction data for the external program
        };

        // Execute the CPI instruction
        invoke(
            &ix,
            &[
                ctx.accounts.loan_vault.to_account_info(),
                ctx.accounts.borrower_account.to_account_info(),
                // Add other account_infos as needed
            ],
        )?;

        //  Borrower repays loan
        let fee = calculate_dynamic_fee(loan_amount); // Calculate fee based on loan size
        let total_repayment = loan_amount + fee;

        // Ensure borrower repays the correct loan amount and fee
        let repayment_amount = ctx.accounts.borrower_account.amount;
        require!(repayment_amount == total_repayment, FlashLoanError::IncorrectRepayment);

        token::transfer(
            ctx.accounts.into_transfer_to_vault_context(),
            total_repayment,
        )?;

        // Update loan stats
        ctx.accounts.loan_stats.update_stats(loan_amount, fee);

        // Update loan state to prevent abuse
        ctx.accounts.loan_state.active = false;
        ctx.accounts.loan_state.last_loan_timestamp = clock.unix_timestamp; // Update cooldown

        // Emit loan execution event
        emit!(FlashLoanExecuted {
            borrower: *ctx.accounts.borrower.key,
            loan_amount,
            fee,
        });

        Ok(())
    }
}

// Context for flash loan
#[derive(Accounts)]
pub struct ExecuteFlashLoan<'info> {
    #[account(mut)]
    pub loan_vault: Account<'info, TokenAccount>,   // Flash loan pool
    #[account(mut)]
    pub borrower_account: Account<'info, TokenAccount>,  // Borrower’s token account
    #[account(mut)]
    pub borrower: Signer<'info>,                   // Borrower signing the transaction
    pub token_program: Program<'info, Token>,      // Token program
    #[account(mut)]
    pub loan_stats: Account<'info, LoanStats>,     // Loan statistics account
    #[account(mut)]
    pub loan_state: Account<'info, LoanState>,     // Reentrancy check and state
    pub token_mint: Account<'info, Mint>,          // Token mint for multi-token support
}

// Loan statistics account
#[account]
pub struct LoanStats {
    pub total_loans: u64,
    pub total_fees_collected: u64,
    pub total_loan_count: u64,      // Number of loans taken
    pub average_loan_size: u64,     // Average loan size
}

impl LoanStats {
    pub fn update_stats(&mut self, loan_amount: u64, fee: u64) {
        self.total_loans += loan_amount;
        self.total_fees_collected += fee;
        self.total_loan_count += 1;
        self.average_loan_size = self.total_loans / self.total_loan_count;
    }
}

// Loan state for reentrancy guard and cooldown tracking
#[account]
pub struct LoanState {
    pub active: bool,               // Whether a loan is currently active
    pub last_loan_timestamp: i64,   // Track when the last loan was issued
}

impl<'info> ExecuteFlashLoan<'info> {
    // Context for transferring tokens to borrower
    pub fn into_transfer_to_borrower_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.loan_vault.to_account_info().clone(),
            to: self.borrower_account.to_account_info().clone(),
            authority: self.loan_vault.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    // Context for borrower repaying the loan
    pub fn into_transfer_to_vault_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.borrower_account.to_account_info().clone(),
            to: self.loan_vault.to_account_info().clone(),
            authority: self.borrower.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

// Calculate a dynamic fee based on loan amount
fn calculate_dynamic_fee(loan_amount: u64) -> u64 {
    if loan_amount > 500_000 {
        (loan_amount * 25) / 10000 // 0.25% for large loans
    } else if loan_amount > 100_000 {
        (loan_amount * 50) / 10000 // 0.5% for medium loans
    } else {
        (loan_amount * 100) / 10000 // 1% for small loans
    }
}

// Error handling
#[error_code]
pub enum FlashLoanError {
    #[msg("Insufficient funds in the loan vault.")]
    InsufficientFunds,
    #[msg("Borrower did not repay the loan.")]
    LoanNotRepaid,
    #[msg("Invalid fee structure.")]
    InvalidFeeStructure,
    #[msg("Reentrancy detected.")]
    Reentrancy,
    #[msg("Flash loan expired.")]
    LoanExpired,
    #[msg("Loan amount exceeds the maximum allowed.")]
    LoanAmountTooLarge,
    #[msg("Borrower repaid an incorrect amount.")]
    IncorrectRepayment,
    #[msg("Cooldown period not over.")]
    CooldownPeriodNotOver,
}

// Flash loan executed event
#[event]
pub struct FlashLoanExecuted {
    pub borrower: Pubkey,
    pub loan_amount: u64,
    pub fee: u64,
}

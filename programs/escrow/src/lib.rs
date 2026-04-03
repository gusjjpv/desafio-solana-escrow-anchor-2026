use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, Transfer};

declare_id!("4PVBfd185KQ7UV3y1h2jR5ydAP9LeB317PNn8aS6gccT");

#[program]
pub mod escrow {
    use super::*;

    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        seed: u64,
        amount_party_one: u64,
        amount_party_two: u64,
        beneficiary: Pubkey,
    ) -> Result<()> {
        require!(amount_party_one > 0, EscrowError::InvalidAmount);
        require!(amount_party_two > 0, EscrowError::InvalidAmount);

        let escrow = &mut ctx.accounts.escrow;
        escrow.initializer = ctx.accounts.initializer.key();
        escrow.counterparty = ctx.accounts.counterparty.key();
        escrow.beneficiary = beneficiary;
        escrow.mint = ctx.accounts.mint.key();
        escrow.seed = seed;
        escrow.amount_party_one = amount_party_one;
        escrow.amount_party_two = amount_party_two;
        escrow.deposited_party_one = 0;
        escrow.deposited_party_two = 0;
        escrow.confirmed_party_one = false;
        escrow.confirmed_party_two = false;
        escrow.bump = ctx.bumps.escrow;
        escrow.vault_bump = ctx.bumps.vault;

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, EscrowError::InvalidAmount);

        let escrow = &mut ctx.accounts.escrow;
        let depositor = ctx.accounts.depositor.key();

        if depositor == escrow.initializer {
            let next_total = escrow
                .deposited_party_one
                .checked_add(amount)
                .ok_or(EscrowError::ArithmeticOverflow)?;
            require!(
                next_total <= escrow.amount_party_one,
                EscrowError::DepositExceedsRequiredAmount
            );
            escrow.deposited_party_one = next_total;
        } else if depositor == escrow.counterparty {
            let next_total = escrow
                .deposited_party_two
                .checked_add(amount)
                .ok_or(EscrowError::ArithmeticOverflow)?;
            require!(
                next_total <= escrow.amount_party_two,
                EscrowError::DepositExceedsRequiredAmount
            );
            escrow.deposited_party_two = next_total;
        } else {
            return err!(EscrowError::UnauthorizedDepositor);
        }

        let cpi_accounts = Transfer {
            from: ctx.accounts.depositor_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.depositor.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        Ok(())
    }

    pub fn confirm_deposit(ctx: Context<ConfirmDeposit>) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let signer = ctx.accounts.signer.key();

        if signer == escrow.initializer {
            require!(
                escrow.deposited_party_one == escrow.amount_party_one,
                EscrowError::DepositNotComplete
            );
            escrow.confirmed_party_one = true;
        } else if signer == escrow.counterparty {
            require!(
                escrow.deposited_party_two == escrow.amount_party_two,
                EscrowError::DepositNotComplete
            );
            escrow.confirmed_party_two = true;
        } else {
            return err!(EscrowError::UnauthorizedSigner);
        }

        Ok(())
    }

    pub fn release_funds(ctx: Context<ReleaseFunds>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        let caller = ctx.accounts.caller.key();

        require!(
            caller == escrow.initializer || caller == escrow.counterparty,
            EscrowError::UnauthorizedSigner
        );
        require!(
            escrow.confirmed_party_one && escrow.confirmed_party_two,
            EscrowError::BothPartiesMustConfirm
        );

        let total_amount = escrow
            .deposited_party_one
            .checked_add(escrow.deposited_party_two)
            .ok_or(EscrowError::ArithmeticOverflow)?;

        let seed_bytes = escrow.seed.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"escrow",
            escrow.initializer.as_ref(),
            seed_bytes.as_ref(),
            &[escrow.bump],
        ];
        let signer = &[signer_seeds];

        let transfer_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let transfer_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(transfer_program, transfer_accounts, signer),
            total_amount,
        )?;

        let close_accounts = CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.initializer.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let close_program = ctx.accounts.token_program.to_account_info();
        token::close_account(CpiContext::new_with_signer(
            close_program,
            close_accounts,
            signer,
        ))?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,

    pub counterparty: SystemAccount<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [b"escrow", initializer.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        init,
        payer = initializer,
        seeds = [b"vault", escrow.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = escrow
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"escrow", escrow.initializer.as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = depositor
    )]
    pub depositor_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", escrow.key().as_ref()],
        bump = escrow.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(address = escrow.mint @ EscrowError::InvalidMint)]
    pub mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct ConfirmDeposit<'info> {
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"escrow", escrow.initializer.as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,
}

#[derive(Accounts)]
pub struct ReleaseFunds<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        close = initializer,
        seeds = [b"escrow", escrow.initializer.as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        mut,
        seeds = [b"vault", escrow.key().as_ref()],
        bump = escrow.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = recipient_token_account.mint == escrow.mint @ EscrowError::InvalidMint,
        constraint = recipient_token_account.owner == escrow.beneficiary @ EscrowError::InvalidBeneficiary
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    #[account(mut, address = escrow.initializer @ EscrowError::InvalidInitializer)]
    pub initializer: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub initializer: Pubkey,
    pub counterparty: Pubkey,
    pub beneficiary: Pubkey,
    pub mint: Pubkey,
    pub seed: u64,
    pub amount_party_one: u64,
    pub amount_party_two: u64,
    pub deposited_party_one: u64,
    pub deposited_party_two: u64,
    pub confirmed_party_one: bool,
    pub confirmed_party_two: bool,
    pub bump: u8,
    pub vault_bump: u8,
}

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Math overflow")]
    ArithmeticOverflow,
    #[msg("Depositor is not part of this escrow")]
    UnauthorizedDepositor,
    #[msg("Signer is not part of this escrow")]
    UnauthorizedSigner,
    #[msg("Deposit exceeds the required amount")]
    DepositExceedsRequiredAmount,
    #[msg("Required deposit is not complete")]
    DepositNotComplete,
    #[msg("Both parties must confirm before release")]
    BothPartiesMustConfirm,
    #[msg("Provided mint does not match escrow mint")]
    InvalidMint,
    #[msg("Recipient token account owner does not match beneficiary")]
    InvalidBeneficiary,
    #[msg("Initializer account does not match escrow initializer")]
    InvalidInitializer,
}

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::TokenStarterError;

#[derive(Accounts)]
pub struct TransferTokens<'info> {
    pub authority: Signer<'info>,
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub source: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
        constraint = source.key() != destination.key() @ TokenStarterError::SourceEqualsDestination,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<TransferTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, TokenStarterError::AmountMustBePositive);

    let decimals = ctx.accounts.mint.decimals;
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.mint.to_account_info(),
        from: ctx.accounts.source.to_account_info(),
        to: ctx.accounts.destination.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi_context = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
    token_interface::transfer_checked(cpi_context, amount, decimals)
}

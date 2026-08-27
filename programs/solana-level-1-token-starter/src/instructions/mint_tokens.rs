use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface};

use crate::error::TokenStarterError;

#[derive(Accounts)]
pub struct MintTokens<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, TokenStarterError::AmountMustBePositive);

    let cpi_accounts = MintTo {
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.destination.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi_context = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
    token_interface::mint_to(cpi_context, amount)
}

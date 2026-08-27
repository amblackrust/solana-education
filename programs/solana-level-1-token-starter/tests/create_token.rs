use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, ID as ASSOCIATED_TOKEN_PROGRAM,
    },
    token_2022::{self, spl_token_2022},
};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

const DECIMALS: u8 = 6;
const MINT_AMOUNT: u64 = 1_000_000;
const TRANSFER_AMOUNT: u64 = 250_000;
const AIRDROP_LAMPORTS: u64 = 1_000_000_000;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Build with `anchor build --ignore-keys` first. Could not read {}: {error}",
            path.display()
        )
    })
}

struct Fixture {
    svm: LiteSVM,
    payer: Keypair,
    authority: Keypair,
    mint: Keypair,
}

impl Fixture {
    fn new() -> Self {
        let payer = Keypair::new();
        let authority = Keypair::new();
        let mint = Keypair::new();
        let mut svm = LiteSVM::new();
        svm.add_program(solana_level_1_token_starter::ID, &program_bytes())
            .expect("program must load");
        svm.airdrop(&payer.pubkey(), AIRDROP_LAMPORTS)
            .expect("airdrop must succeed");
        let mut fixture = Self {
            svm,
            payer,
            authority,
            mint,
        };
        fixture.create_mint();
        fixture
    }

    fn create_mint(&mut self) {
        let accounts = solana_level_1_token_starter::accounts::CreateToken {
            payer: self.payer.pubkey(),
            authority: self.authority.pubkey(),
            mint: self.mint.pubkey(),
            token_program: token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        };
        let ix = instruction(
            solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS }.data(),
            accounts.to_account_metas(None),
        );
        send(
            &mut self.svm,
            ix,
            &[&self.payer, &self.authority, &self.mint],
        )
        .expect("create_token must succeed");
    }

    fn token_account(&self, owner: &anchor_lang::prelude::Pubkey) -> anchor_lang::prelude::Pubkey {
        get_associated_token_address_with_program_id(owner, &self.mint.pubkey(), &token_2022::ID)
    }

    fn create_token_account(&mut self, owner: &anchor_lang::prelude::Pubkey) {
        let accounts = solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: self.payer.pubkey(),
            owner: *owner,
            mint: self.mint.pubkey(),
            token_account: self.token_account(owner),
            token_program: token_2022::ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM,
            system_program: anchor_lang::system_program::ID,
        };
        let ix = instruction(
            solana_level_1_token_starter::instruction::CreateTokenAccount {}.data(),
            accounts.to_account_metas(None),
        );
        send(&mut self.svm, ix, &[&self.payer]).expect("create_token_account must succeed");
    }
}

fn instruction(data: Vec<u8>, metas: Vec<anchor_lang::prelude::AccountMeta>) -> Instruction {
    Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: metas
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data,
    }
}

fn send(svm: &mut LiteSVM, instruction: Instruction, signers: &[&Keypair]) -> Result<(), String> {
    let payer = signers.first().expect("transaction needs a fee payer");
    let transaction = Transaction::new(
        signers,
        Message::new(&[instruction], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(transaction)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

fn mint_state(
    svm: &LiteSVM,
    address: &anchor_lang::prelude::Pubkey,
) -> spl_token_2022::state::Mint {
    let account = svm.get_account(address).expect("mint must exist");
    spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
        &account.data,
    )
    .expect("mint data must decode")
    .base
}

fn token_balance(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> u64 {
    let account = svm.get_account(address).expect("token account must exist");
    spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
        &account.data,
    )
    .expect("token account data must decode")
    .base
    .amount
}

#[test]
fn creates_token_2022_mint_with_expected_state() {
    let fixture = Fixture::new();
    let mint = mint_state(&fixture.svm, &fixture.mint.pubkey());
    assert_eq!(
        fixture
            .svm
            .get_account(&fixture.mint.pubkey())
            .unwrap()
            .owner,
        token_2022::ID
    );
    assert_eq!(mint.decimals, DECIMALS);
    assert_eq!(mint.mint_authority, Some(fixture.authority.pubkey()).into());
    assert_eq!(mint.supply, 0);
}

#[test]
fn creates_associated_token_account_with_expected_owner_mint_and_program() {
    let mut fixture = Fixture::new();
    let owner = Keypair::new();
    fixture.create_token_account(&owner.pubkey());
    let address = fixture.token_account(&owner.pubkey());
    let account = fixture
        .svm
        .get_account(&address)
        .expect("token account must exist");
    let state =
        spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
            &account.data,
        )
        .expect("token account data must decode")
        .base;
    assert_eq!(account.owner, token_2022::ID);
    assert_eq!(state.owner, owner.pubkey());
    assert_eq!(state.mint, fixture.mint.pubkey());
}

fn mint_to(
    fixture: &mut Fixture,
    destination: anchor_lang::prelude::Pubkey,
    amount: u64,
) -> Result<(), String> {
    let accounts = solana_level_1_token_starter::accounts::MintTokens {
        authority: fixture.authority.pubkey(),
        mint: fixture.mint.pubkey(),
        destination,
        token_program: token_2022::ID,
    };
    send(
        &mut fixture.svm,
        instruction(
            solana_level_1_token_starter::instruction::MintTokens { amount }.data(),
            accounts.to_account_metas(None),
        ),
        &[&fixture.payer, &fixture.authority],
    )
}

#[test]
fn mint_tokens_changes_recipient_balance_and_supply() {
    let mut fixture = Fixture::new();
    let recipient = Keypair::new();
    fixture.create_token_account(&recipient.pubkey());
    let destination = fixture.token_account(&recipient.pubkey());
    mint_to(&mut fixture, destination, MINT_AMOUNT).expect("mint_tokens must succeed");
    assert_eq!(token_balance(&fixture.svm, &destination), MINT_AMOUNT);
    assert_eq!(
        mint_state(&fixture.svm, &fixture.mint.pubkey()).supply,
        MINT_AMOUNT
    );
}

#[test]
fn transfer_tokens_changes_both_balances_without_changing_supply() {
    let mut fixture = Fixture::new();
    let recipient = Keypair::new();
    let payer_address = fixture.payer.pubkey();
    fixture.create_token_account(&payer_address);
    fixture.create_token_account(&recipient.pubkey());
    let source = fixture.token_account(&fixture.payer.pubkey());
    let destination = fixture.token_account(&recipient.pubkey());
    mint_to(&mut fixture, source, MINT_AMOUNT).expect("mint_tokens must succeed");
    let supply_before = mint_state(&fixture.svm, &fixture.mint.pubkey()).supply;
    let accounts = solana_level_1_token_starter::accounts::TransferTokens {
        authority: fixture.payer.pubkey(),
        mint: fixture.mint.pubkey(),
        source,
        destination,
        token_program: token_2022::ID,
    };
    send(
        &mut fixture.svm,
        instruction(
            solana_level_1_token_starter::instruction::TransferTokens {
                amount: TRANSFER_AMOUNT,
            }
            .data(),
            accounts.to_account_metas(None),
        ),
        &[&fixture.payer],
    )
    .expect("transfer_tokens must succeed");
    assert_eq!(
        token_balance(&fixture.svm, &source),
        MINT_AMOUNT - TRANSFER_AMOUNT
    );
    assert_eq!(token_balance(&fixture.svm, &destination), TRANSFER_AMOUNT);
    assert_eq!(
        mint_state(&fixture.svm, &fixture.mint.pubkey()).supply,
        supply_before
    );
}

#[test]
fn rejects_zero_amount_wrong_authority_wrong_mint_and_same_accounts() {
    let mut fixture = Fixture::new();
    let recipient = Keypair::new();
    let payer_address = fixture.payer.pubkey();
    fixture.create_token_account(&payer_address);
    fixture.create_token_account(&recipient.pubkey());
    let source = fixture.token_account(&fixture.payer.pubkey());
    let destination = fixture.token_account(&recipient.pubkey());
    let zero_accounts = solana_level_1_token_starter::accounts::MintTokens {
        authority: fixture.authority.pubkey(),
        mint: fixture.mint.pubkey(),
        destination: source,
        token_program: token_2022::ID,
    };
    assert!(
        send(
            &mut fixture.svm,
            instruction(
                solana_level_1_token_starter::instruction::MintTokens { amount: 0 }.data(),
                zero_accounts.to_account_metas(None)
            ),
            &[&fixture.payer, &fixture.authority]
        )
        .is_err(),
        "zero amount must fail"
    );
    let wrong_authority = Keypair::new();
    let wrong_accounts = solana_level_1_token_starter::accounts::MintTokens {
        authority: wrong_authority.pubkey(),
        mint: fixture.mint.pubkey(),
        destination: source,
        token_program: token_2022::ID,
    };
    assert!(
        send(
            &mut fixture.svm,
            instruction(
                solana_level_1_token_starter::instruction::MintTokens { amount: 1 }.data(),
                wrong_accounts.to_account_metas(None)
            ),
            &[&fixture.payer, &wrong_authority]
        )
        .is_err(),
        "wrong authority must fail"
    );
    let other_mint = Keypair::new();
    let other_accounts = solana_level_1_token_starter::accounts::CreateToken {
        payer: fixture.payer.pubkey(),
        authority: fixture.authority.pubkey(),
        mint: other_mint.pubkey(),
        token_program: token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };
    send(
        &mut fixture.svm,
        instruction(
            solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS }.data(),
            other_accounts.to_account_metas(None),
        ),
        &[&fixture.payer, &fixture.authority, &other_mint],
    )
    .expect("second mint must be created");
    let wrong_mint_accounts = solana_level_1_token_starter::accounts::MintTokens {
        authority: fixture.authority.pubkey(),
        mint: other_mint.pubkey(),
        destination,
        token_program: token_2022::ID,
    };
    assert!(
        send(
            &mut fixture.svm,
            instruction(
                solana_level_1_token_starter::instruction::MintTokens { amount: 1 }.data(),
                wrong_mint_accounts.to_account_metas(None)
            ),
            &[&fixture.payer, &fixture.authority]
        )
        .is_err(),
        "wrong mint/account pair must fail"
    );
    let same_accounts = solana_level_1_token_starter::accounts::TransferTokens {
        authority: fixture.payer.pubkey(),
        mint: fixture.mint.pubkey(),
        source,
        destination: source,
        token_program: token_2022::ID,
    };
    assert!(
        send(
            &mut fixture.svm,
            instruction(
                solana_level_1_token_starter::instruction::TransferTokens { amount: 1 }.data(),
                same_accounts.to_account_metas(None)
            ),
            &[&fixture.payer]
        )
        .is_err(),
        "same accounts must fail"
    );
}

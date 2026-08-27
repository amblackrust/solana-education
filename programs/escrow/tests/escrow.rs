use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, ID as ASSOCIATED_TOKEN_PROGRAM,
    },
    token_2022::{self, spl_token_2022},
};
use escrow::EscrowStatus;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_level_1_token_starter as token_starter;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

const DECIMALS: u8 = 6;
const DEAL_ID: u64 = 7;
const DEAL_AMOUNT: u64 = 800_000;
const AIRDROP: u64 = 2_000_000_000;

fn bytes(name: &str) -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/deploy/{name}.so"));
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Build SBF first; could not read {}: {error}",
            path.display()
        )
    })
}

struct Fixture {
    svm: LiteSVM,
    sender: Keypair,
    receiver: Keypair,
    mint_authority: Keypair,
    mint: Keypair,
    sender_token: anchor_lang::prelude::Pubkey,
    escrow: anchor_lang::prelude::Pubkey,
    vault: anchor_lang::prelude::Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let sender = Keypair::new();
        let receiver = Keypair::new();
        let mint_authority = Keypair::new();
        let mint = Keypair::new();
        let mut svm = LiteSVM::new();
        svm.add_program(escrow::ID, &bytes("escrow"))
            .expect("escrow must load");
        svm.add_program(token_starter::ID, &bytes("solana_level_1_token_starter"))
            .expect("token starter must load");
        svm.airdrop(&sender.pubkey(), AIRDROP)
            .expect("sender airdrop");
        svm.airdrop(&receiver.pubkey(), AIRDROP)
            .expect("receiver airdrop");
        let mut fixture = Self {
            svm,
            sender,
            receiver,
            mint_authority,
            mint,
            sender_token: anchor_lang::prelude::Pubkey::default(),
            escrow: anchor_lang::prelude::Pubkey::default(),
            vault: anchor_lang::prelude::Pubkey::default(),
        };
        fixture.create_mint();
        fixture.sender_token = ata(&fixture.sender.pubkey(), &fixture.mint.pubkey());
        fixture.create_sender_ata();
        fixture.mint_tokens();
        (fixture.escrow, _) = anchor_lang::prelude::Pubkey::find_program_address(
            &[
                b"escrow",
                fixture.sender.pubkey().as_ref(),
                &DEAL_ID.to_le_bytes(),
            ],
            &escrow::ID,
        );
        (fixture.vault, _) = anchor_lang::prelude::Pubkey::find_program_address(
            &[b"vault", fixture.escrow.as_ref()],
            &escrow::ID,
        );
        fixture
    }

    fn create_mint(&mut self) {
        let accounts = token_starter::accounts::CreateToken {
            payer: self.sender.pubkey(),
            authority: self.mint_authority.pubkey(),
            mint: self.mint.pubkey(),
            token_program: token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        };
        send(
            &mut self.svm,
            token_ix(
                token_starter::instruction::CreateToken { decimals: DECIMALS }.data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender, &self.mint_authority, &self.mint],
        )
        .expect("create mint");
    }

    fn create_sender_ata(&mut self) {
        let accounts = token_starter::accounts::CreateTokenAccount {
            payer: self.sender.pubkey(),
            owner: self.sender.pubkey(),
            mint: self.mint.pubkey(),
            token_account: self.sender_token,
            token_program: token_2022::ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM,
            system_program: anchor_lang::system_program::ID,
        };
        send(
            &mut self.svm,
            token_ix(
                token_starter::instruction::CreateTokenAccount {}.data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender],
        )
        .expect("create sender ATA");
    }

    fn mint_tokens(&mut self) {
        let accounts = token_starter::accounts::MintTokens {
            authority: self.mint_authority.pubkey(),
            mint: self.mint.pubkey(),
            destination: self.sender_token,
            token_program: token_2022::ID,
        };
        send(
            &mut self.svm,
            token_ix(
                token_starter::instruction::MintTokens {
                    amount: DEAL_AMOUNT,
                }
                .data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender, &self.mint_authority],
        )
        .expect("mint tokens");
    }

    fn create_other_mint(&mut self, mint: &Keypair) {
        let accounts = token_starter::accounts::CreateToken {
            payer: self.sender.pubkey(),
            authority: self.mint_authority.pubkey(),
            mint: mint.pubkey(),
            token_program: token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        };
        send(
            &mut self.svm,
            token_ix(
                token_starter::instruction::CreateToken { decimals: DECIMALS }.data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender, &self.mint_authority, mint],
        )
        .expect("create other mint");
    }

    fn initialize(
        &mut self,
        amount: u64,
        receiver: anchor_lang::prelude::Pubkey,
    ) -> Result<(), String> {
        let accounts = escrow::accounts::Initialize {
            sender: self.sender.pubkey(),
            receiver,
            mint: self.mint.pubkey(),
            escrow_state: self.escrow,
            vault: self.vault,
            token_program: token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        };
        send(
            &mut self.svm,
            ix(
                escrow::instruction::Initialize {
                    deal_id: DEAL_ID,
                    amount,
                }
                .data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender],
        )
    }

    fn deposit(&mut self) -> Result<(), String> {
        let accounts = escrow::accounts::Deposit {
            sender: self.sender.pubkey(),
            escrow_state: self.escrow,
            mint: self.mint.pubkey(),
            sender_token_account: self.sender_token,
            vault: self.vault,
            token_program: token_2022::ID,
        };
        send(
            &mut self.svm,
            ix(
                escrow::instruction::Deposit {}.data(),
                accounts.to_account_metas(None),
            ),
            &[&self.sender],
        )
    }
}

fn ata(
    owner: &anchor_lang::prelude::Pubkey,
    mint: &anchor_lang::prelude::Pubkey,
) -> anchor_lang::prelude::Pubkey {
    get_associated_token_address_with_program_id(owner, mint, &token_2022::ID)
}

fn ix(data: Vec<u8>, metas: Vec<anchor_lang::prelude::AccountMeta>) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: metas
            .into_iter()
            .map(|m| AccountMeta {
                pubkey: m.pubkey,
                is_signer: m.is_signer,
                is_writable: m.is_writable,
            })
            .collect(),
        data,
    }
}

fn token_ix(data: Vec<u8>, metas: Vec<anchor_lang::prelude::AccountMeta>) -> Instruction {
    Instruction {
        program_id: token_starter::ID,
        accounts: metas
            .into_iter()
            .map(|m| AccountMeta {
                pubkey: m.pubkey,
                is_signer: m.is_signer,
                is_writable: m.is_writable,
            })
            .collect(),
        data,
    }
}

fn send(svm: &mut LiteSVM, instruction: Instruction, signers: &[&Keypair]) -> Result<(), String> {
    let payer = signers.first().expect("fee payer");
    svm.send_transaction(Transaction::new(
        signers,
        Message::new(&[instruction], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    ))
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

fn balance(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> u64 {
    let account = svm.get_account(address).expect("token account");
    spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
        &account.data,
    )
    .expect("token account data")
    .base
    .amount
}

fn supply(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> u64 {
    let account = svm.get_account(address).expect("mint account");
    spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
        &account.data,
    )
    .expect("mint data")
    .base
    .supply
}

fn state(svm: &LiteSVM, address: &anchor_lang::prelude::Pubkey) -> escrow::EscrowState {
    let account = svm.get_account(address).expect("escrow state");
    let mut data = account.data.as_slice();
    escrow::EscrowState::try_deserialize(&mut data).expect("escrow state data")
}

#[test]
fn release_transfers_exact_amount_and_closes_state_and_vault() {
    let mut f = Fixture::new();
    f.initialize(DEAL_AMOUNT, f.receiver.pubkey())
        .expect("initialize");
    f.deposit().expect("deposit");
    let receiver_token = ata(&f.receiver.pubkey(), &f.mint.pubkey());
    let supply_before = supply(&f.svm, &f.mint.pubkey());
    let accounts = escrow::accounts::Release {
        sender: f.sender.pubkey(),
        escrow_state: f.escrow,
        receiver: f.receiver.pubkey(),
        mint: f.mint.pubkey(),
        vault: f.vault,
        receiver_token_account: receiver_token,
        token_program: token_2022::ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM,
        system_program: anchor_lang::system_program::ID,
    };
    send(
        &mut f.svm,
        ix(
            escrow::instruction::Release {}.data(),
            accounts.to_account_metas(None),
        ),
        &[&f.sender],
    )
    .expect("release");
    assert_eq!(balance(&f.svm, &receiver_token), DEAL_AMOUNT);
    assert_eq!(supply(&f.svm, &f.mint.pubkey()), supply_before);
    assert!(f.svm.get_account(&f.vault).is_none());
    assert!(f.svm.get_account(&f.escrow).is_none());
}

#[test]
fn cancel_returns_funds_and_closes_state_and_vault() {
    let mut f = Fixture::new();
    f.initialize(DEAL_AMOUNT, f.receiver.pubkey())
        .expect("initialize");
    f.deposit().expect("deposit");
    let balance_before = balance(&f.svm, &f.sender_token);
    let supply_before = supply(&f.svm, &f.mint.pubkey());
    let accounts = escrow::accounts::Cancel {
        sender: f.sender.pubkey(),
        escrow_state: f.escrow,
        mint: f.mint.pubkey(),
        sender_token_account: f.sender_token,
        vault: f.vault,
        token_program: token_2022::ID,
    };
    send(
        &mut f.svm,
        ix(
            escrow::instruction::Cancel {}.data(),
            accounts.to_account_metas(None),
        ),
        &[&f.sender],
    )
    .expect("cancel");
    assert_eq!(
        balance(&f.svm, &f.sender_token),
        balance_before + DEAL_AMOUNT
    );
    assert_eq!(supply(&f.svm, &f.mint.pubkey()), supply_before);
    assert!(f.svm.get_account(&f.vault).is_none());
    assert!(f.svm.get_account(&f.escrow).is_none());
}

#[test]
fn rejects_invalid_initialize_deposit_and_completion_without_state_changes() {
    let mut f = Fixture::new();
    assert!(f.initialize(0, f.receiver.pubkey()).is_err());
    assert!(f.initialize(DEAL_AMOUNT, f.sender.pubkey()).is_err());
    f.initialize(DEAL_AMOUNT, f.receiver.pubkey())
        .expect("initialize");
    assert!(
        f.initialize(DEAL_AMOUNT, f.receiver.pubkey()).is_err(),
        "repeated deal id must fail"
    );
    assert!(f.deposit().is_ok());
    let funded = state(&f.svm, &f.escrow);
    let balance_before = balance(&f.svm, &f.sender_token);
    let supply_before = supply(&f.svm, &f.mint.pubkey());
    assert_eq!(funded.status, EscrowStatus::Funded);
    assert!(f.deposit().is_err());
    assert_eq!(state(&f.svm, &f.escrow).status, EscrowStatus::Funded);
    let wrong = Keypair::new();
    f.svm
        .airdrop(&wrong.pubkey(), AIRDROP)
        .expect("wrong signer funding");
    let accounts = escrow::accounts::Release {
        sender: wrong.pubkey(),
        escrow_state: f.escrow,
        receiver: f.receiver.pubkey(),
        mint: f.mint.pubkey(),
        vault: f.vault,
        receiver_token_account: ata(&f.receiver.pubkey(), &f.mint.pubkey()),
        token_program: token_2022::ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM,
        system_program: anchor_lang::system_program::ID,
    };
    assert!(send(
        &mut f.svm,
        ix(
            escrow::instruction::Release {}.data(),
            accounts.to_account_metas(None)
        ),
        &[&wrong]
    )
    .is_err());
    assert_eq!(state(&f.svm, &f.escrow).status, EscrowStatus::Funded);
    assert_eq!(balance(&f.svm, &f.sender_token), balance_before);
    assert_eq!(supply(&f.svm, &f.mint.pubkey()), supply_before);

    let other_mint = Keypair::new();
    f.create_other_mint(&other_mint);
    let wrong_mint_accounts = escrow::accounts::Deposit {
        sender: f.sender.pubkey(),
        escrow_state: f.escrow,
        mint: other_mint.pubkey(),
        sender_token_account: f.sender_token,
        vault: f.vault,
        token_program: token_2022::ID,
    };
    assert!(
        send(
            &mut f.svm,
            ix(
                escrow::instruction::Deposit {}.data(),
                wrong_mint_accounts.to_account_metas(None),
            ),
            &[&f.sender],
        )
        .is_err(),
        "different mint must fail"
    );
    assert_eq!(state(&f.svm, &f.escrow).status, EscrowStatus::Funded);

    let mut insufficient = Fixture::new();
    insufficient
        .initialize(DEAL_AMOUNT + 1, insufficient.receiver.pubkey())
        .expect("initialize insufficient-balance deal");
    let insufficient_balance = balance(&insufficient.svm, &insufficient.sender_token);
    assert!(
        insufficient.deposit().is_err(),
        "insufficient balance must fail"
    );
    assert_eq!(
        balance(&insufficient.svm, &insufficient.sender_token),
        insufficient_balance
    );
    assert_eq!(
        state(&insufficient.svm, &insufficient.escrow).status,
        EscrowStatus::Created
    );
}

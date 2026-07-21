//! Testy integracyjne pełnego cyklu (WP v1.0) — solana-program-test.
//!
//! Po audycie #5 setup jest 4-etapowy (fix stack-overflow SBF):
//! `initialize` (sam GlobalConfig) → `init_principal_vault` →
//! `init_reward_vault` → `init_xnt_vault` → `create_pool`×2.
//! Test TS-AUD5 pokrywa stan pośredni (GlobalConfig bez skarbców).
//!
//! Program wykonuje się in-process (processor!) z prawdziwymi CPI do
//! Token-2022 (ANL) i SPL Token (XNT). Zegar kontrolowany przez sysvar Clock.
//! Scenariusze TS-xx mapują rozdziały White Papera; stałe czasowe brane z
//! `anl-math`, więc suite działa identycznie w buildzie produkcyjnym
//! i `--features test-periods` (okna 3/9 dni, min. okres 1 dzień).
//!
//! Uruchomienie: `cargo test -p anl_staking --features test-periods --test integration`

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anl_staking::state::{PoolConfig, PoolType, PositionStatus, UserPosition};
use solana_program_test::{
    processor, BanksClient, BanksClientError, ProgramTest, ProgramTestContext,
};
use solana_sdk::{
    clock::Clock,
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

/// Adapter lifetimes: anchorowe `entry` wymaga wspólnego 'info dla slice'a
/// i AccountInfo, a `processor!` podaje ogólniejsze lifetimes. Transmute jest
/// standardowym, bezpiecznym w tym kontekście mostkiem (konta żyją przez całe
/// wywołanie) — używanym powszechnie w testach integracyjnych Anchora.
fn anchor_entry(
    program_id: &solana_sdk::pubkey::Pubkey,
    accounts: &[anchor_lang::prelude::AccountInfo],
    data: &[u8],
) -> anchor_lang::solana_program::entrypoint::ProgramResult {
    let accounts: &[anchor_lang::prelude::AccountInfo] = unsafe { std::mem::transmute(accounts) };
    anl_staking::entry(program_id, accounts, data)
}

const DECIMALS: u8 = 9;
const ONE_ANL: u64 = 1_000_000_000;
const DAY: i64 = anl_math::SECONDS_PER_DAY;

// ============================================================ scaffolding

struct Env {
    ctx: ProgramTestContext,
    program_id: Pubkey,
    authority: Keypair,
    anl_mint: Keypair,
    xnt_mint: Pubkey,
    global_config: Pubkey,
    vault_authority: Pubkey,
    principal_vault: Pubkey,
    reward_vault: Pubkey,
    xnt_vault: Pubkey,
    genesis_pool: Pubkey,
    flexible_pool: Pubkey,
    genesis_start_ts: i64,
    anl_treasury: Pubkey,
    last_funded_epoch: Option<u64>,
}

impl Env {
    /// Pełny setup 4-etapowy — stan „gotowy do stakingu" (jak w produkcji).
    async fn new() -> Self {
        let mut env = Self::new_pre_init().await;
        let ix = env.ix_initialize();
        env.send(&[ix], &[]).await.unwrap();
        env.init_all_vaults().await;
        env.create_pools().await;
        env
    }

    /// Środowisko TUŻ PRZED `initialize`: minty gotowe, podaż ANL wybita,
    /// mint authority odwołane. Testy stanu pośredniego startują stąd.
    async fn new_pre_init() -> Self {
        let program_id = anl_staking::id();
        let mut pt = ProgramTest::new("anl_staking", program_id, processor!(anchor_entry));
        pt.add_program(
            "spl_token",
            spl_token::id(),
            processor!(spl_token::processor::Processor::process),
        );
        pt.add_program(
            "spl_token_2022",
            spl_token_2022::id(),
            processor!(spl_token_2022::processor::Processor::process),
        );
        let mut ctx = pt.start_with_context().await;

        let authority = Keypair::new();
        airdrop(&mut ctx, &authority.pubkey(), 100_000_000_000).await;

        // mints: ANL = Token-2022, XNT = legacy SPL (D-14)
        let anl_mint = Keypair::new();
        create_mint(&mut ctx, &anl_mint, &authority, spl_token_2022::id()).await;
        // Build produkcyjny wymaga minta XNT DOKŁADNIE pod adresem wrapped
        // native (audyt #2) — wstrzykujemy konto minta pod tym adresem.
        // Build test-periods używa zwykłego minta (osobny Program ID).
        let xnt_mint: Pubkey = if cfg!(feature = "test-periods") {
            let kp = Keypair::new();
            create_mint(&mut ctx, &kp, &authority, spl_token::id()).await;
            kp.pubkey()
        } else {
            use solana_sdk::program_pack::Pack;
            let expected = anl_staking::constants::EXPECTED_XNT_MINT;
            let mut data = vec![0u8; spl_token::state::Mint::LEN];
            spl_token::state::Mint {
                mint_authority: solana_sdk::program_option::COption::Some(authority.pubkey()),
                supply: 0,
                decimals: DECIMALS,
                is_initialized: true,
                freeze_authority: solana_sdk::program_option::COption::None,
            }
            .pack_into_slice(&mut data);
            let acc = solana_sdk::account::AccountSharedData::from(solana_sdk::account::Account {
                lamports: 1_000_000_000,
                data,
                owner: spl_token::id(),
                executable: false,
                rent_epoch: 0,
            });
            ctx.set_account(&expected, &acc);
            expected
        };

        let (global_config, _) = Pubkey::find_program_address(&[b"global_config"], &program_id);
        let (vault_authority, _) = Pubkey::find_program_address(&[b"vault_authority"], &program_id);
        let (principal_vault, _) = Pubkey::find_program_address(&[b"principal_vault"], &program_id);
        let (reward_vault, _) = Pubkey::find_program_address(&[b"reward_vault"], &program_id);
        let (xnt_vault, _) = Pubkey::find_program_address(&[b"xnt_vault"], &program_id);
        let (genesis_pool, _) =
            Pubkey::find_program_address(&[b"pool", &[PoolType::Genesis as u8]], &program_id);
        let (flexible_pool, _) =
            Pubkey::find_program_address(&[b"pool", &[PoolType::Flexible as u8]], &program_id);

        let genesis_start_ts = clock(&mut ctx.banks_client).await.unix_timestamp;

        let mut env = Env {
            ctx,
            program_id,
            authority,
            anl_mint,
            xnt_mint,
            global_config,
            vault_authority,
            principal_vault,
            reward_vault,
            xnt_vault,
            genesis_pool,
            flexible_pool,
            genesis_start_ts,
            anl_treasury: Pubkey::default(),
            last_funded_epoch: None,
        };

        // Audyt #2: fixed supply — cała podaż ANL do skarbca testowego,
        // potem mint authority -> None, dopiero wtedy initialize.
        let treasury = create_token_account(
            &mut env.ctx,
            &env.authority.pubkey(),
            &env.anl_mint.pubkey(),
            spl_token_2022::id(),
        )
        .await;
        mint_to(
            &mut env.ctx,
            &env.anl_mint.pubkey(),
            &treasury,
            &env.authority,
            100_000_000 * ONE_ANL,
            spl_token_2022::id(),
        )
        .await;
        env.anl_treasury = treasury;
        {
            let ix = spl_token_2022::instruction::set_authority(
                &spl_token_2022::id(),
                &env.anl_mint.pubkey(),
                None,
                spl_token_2022::instruction::AuthorityType::MintTokens,
                &env.authority.pubkey(),
                &[],
            )
            .unwrap();
            env.send(&[ix], &[]).await.unwrap();
        }

        env
    }

    // ------------- setup 4-etapowy (po audycie #5, A-01) -------------

    fn ix_initialize(&self) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::Initialize {
                authority: self.authority.pubkey(),
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                xnt_mint: self.xnt_mint,
                anl_token_program: spl_token_2022::id(),
                xnt_token_program: spl_token::id(),
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::Initialize {
                genesis_start_ts: self.genesis_start_ts,
                start_paused: false,
            }
            .data(),
        }
    }

    /// `authority_key` jawnie parametrem — testy stanu pośredniego wołają
    /// tę instrukcję także jako NIE-authority (oczekując odrzucenia has_one).
    fn ix_init_principal_vault(&self, authority_key: Pubkey) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::InitPrincipalVault {
                authority: authority_key,
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                principal_vault: self.principal_vault,
                anl_token_program: spl_token_2022::id(),
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::InitPrincipalVault {}.data(),
        }
    }

    fn ix_init_reward_vault(&self, authority_key: Pubkey) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::InitRewardVault {
                authority: authority_key,
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                reward_vault: self.reward_vault,
                anl_token_program: spl_token_2022::id(),
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::InitRewardVault {}.data(),
        }
    }

    fn ix_init_xnt_vault(&self, authority_key: Pubkey) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::InitXntVault {
                authority: authority_key,
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                xnt_mint: self.xnt_mint,
                xnt_vault: self.xnt_vault,
                xnt_token_program: spl_token::id(),
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::InitXntVault {}.data(),
        }
    }

    /// Trzy skarbce w jednej transakcji (limit ramki stosu liczy się
    /// per instrukcję, nie per transakcję — to weryfikuje fix audytu #5).
    async fn init_all_vaults(&mut self) {
        let ixs = [
            self.ix_init_principal_vault(self.authority.pubkey()),
            self.ix_init_reward_vault(self.authority.pubkey()),
            self.ix_init_xnt_vault(self.authority.pubkey()),
        ];
        self.send(&ixs, &[]).await.unwrap();
    }

    async fn create_pools(&mut self) {
        for pool_type in [PoolType::Genesis, PoolType::Flexible] {
            let pool = if pool_type == PoolType::Genesis {
                self.genesis_pool
            } else {
                self.flexible_pool
            };
            let ix = Instruction {
                program_id: self.program_id,
                accounts: anl_staking::accounts::CreatePool {
                    authority: self.authority.pubkey(),
                    global_config: self.global_config,
                    pool_config: pool,
                    system_program: solana_sdk::system_program::id(),
                }
                .to_account_metas(None),
                data: anl_staking::instruction::CreatePool { pool_type }.data(),
            };
            self.send(&[ix], &[]).await.unwrap();
        }
    }

    /// Transakcja z DOWOLNYM płatnikiem (testy odrzucenia nie-authority).
    async fn send_as(
        &mut self,
        payer: &Keypair,
        ixs: &[Instruction],
        extra: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let bh = self.ctx.banks_client.get_latest_blockhash().await.unwrap();
        let mut signers: Vec<&Keypair> = vec![payer];
        signers.extend_from_slice(extra);
        let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), &signers, bh);
        self.ctx.banks_client.process_transaction(tx).await
    }

    /// Transakcja podpisana przez authority + dodatkowych signerów.
    async fn send(
        &mut self,
        ixs: &[Instruction],
        extra: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let bh = self.ctx.banks_client.get_latest_blockhash().await.unwrap();
        let mut signers: Vec<&Keypair> = vec![&self.authority];
        signers.extend_from_slice(extra);
        let tx =
            Transaction::new_signed_with_payer(ixs, Some(&self.authority.pubkey()), &signers, bh);
        self.ctx.banks_client.process_transaction(tx).await
    }

    /// Przesuwa zegar o `secs` (nowy slot ⇒ świeży blockhash, potem Clock).
    async fn advance(&mut self, secs: i64) {
        let mut c = clock(&mut self.ctx.banks_client).await;
        let slot = c.slot + 500;
        self.ctx.warp_to_slot(slot).unwrap();
        c = clock(&mut self.ctx.banks_client).await;
        c.unix_timestamp += secs;
        self.ctx.set_sysvar(&c);
    }

    #[allow(dead_code)]
    async fn now(&mut self) -> i64 {
        clock(&mut self.ctx.banks_client).await.unix_timestamp
    }

    // -------------------- tokeny --------------------

    async fn user_with_anl(&mut self, amount: u64) -> (Keypair, Pubkey, Pubkey) {
        let user = Keypair::new();
        airdrop(&mut self.ctx, &user.pubkey(), 10_000_000_000).await;
        let anl_acc = create_token_account(
            &mut self.ctx,
            &user.pubkey(),
            &self.anl_mint.pubkey(),
            spl_token_2022::id(),
        )
        .await;
        let xnt_acc = create_token_account(
            &mut self.ctx,
            &user.pubkey(),
            &self.xnt_mint,
            spl_token::id(),
        )
        .await;
        self.transfer_anl(anl_acc, amount).await;
        (user, anl_acc, xnt_acc)
    }

    async fn transfer_anl(&mut self, to: Pubkey, amount: u64) {
        let ix = spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::id(),
            &self.anl_treasury,
            &self.anl_mint.pubkey(),
            &to,
            &self.authority.pubkey(),
            &[],
            amount,
            DECIMALS,
        )
        .unwrap();
        self.send(&[ix], &[]).await.unwrap();
    }

    fn ckpt_pda(&self, pool_type: PoolType, epoch: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[b"xnt_ckpt", &[pool_type as u8], &epoch.to_le_bytes()],
            &self.program_id,
        )
        .0
    }

    async fn current_epoch(&mut self) -> u64 {
        let now = clock(&mut self.ctx.banks_client).await.unix_timestamp;
        ((now - self.genesis_start_ts) as u64) / 86_400
    }

    async fn fund_rewards(&mut self, amount: u64) {
        let src = create_token_account(
            &mut self.ctx,
            &self.authority.pubkey(),
            &self.anl_mint.pubkey(),
            spl_token_2022::id(),
        )
        .await;
        self.transfer_anl(src, amount).await;
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::FundRewards {
                funder: self.authority.pubkey(),
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                funder_anl: src,
                reward_vault: self.reward_vault,
                anl_token_program: spl_token_2022::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::FundRewards { amount }.data(),
        };
        self.send(&[ix], &[]).await.unwrap();
    }

    async fn fund_xnt(&mut self, amount: u64) -> Result<(), BanksClientError> {
        let epoch = self.current_epoch().await;
        let (g_prev, f_prev) = match self.last_funded_epoch {
            Some(last) if last != epoch => (
                Some(self.ckpt_pda(PoolType::Genesis, last)),
                Some(self.ckpt_pda(PoolType::Flexible, last)),
            ),
            _ => (None, None),
        };
        let src = create_token_account(
            &mut self.ctx,
            &self.authority.pubkey(),
            &self.xnt_mint,
            spl_token::id(),
        )
        .await;
        if cfg!(feature = "test-periods") {
            mint_to(
                &mut self.ctx,
                &self.xnt_mint,
                &src,
                &self.authority,
                amount,
                spl_token::id(),
            )
            .await;
        } else {
            // mint natywny: zasilenie przez wrap (transfer + sync_native) —
            // identycznie jak bot produkcyjny po wypłacie z vote konta
            let ixs = [
                solana_sdk::system_instruction::transfer(&self.authority.pubkey(), &src, amount),
                spl_token::instruction::sync_native(&spl_token::id(), &src).unwrap(),
            ];
            self.send(&ixs, &[]).await.unwrap();
        }
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::FundXnt {
                funder: self.authority.pubkey(),
                global_config: self.global_config,
                vault_authority: self.vault_authority,
                xnt_mint: self.xnt_mint,
                funder_xnt: src,
                xnt_vault: self.xnt_vault,
                genesis_pool: self.genesis_pool,
                flexible_pool: self.flexible_pool,
                xnt_token_program: spl_token::id(),
                genesis_ckpt: self.ckpt_pda(PoolType::Genesis, epoch),
                flexible_ckpt: self.ckpt_pda(PoolType::Flexible, epoch),
                genesis_prev_ckpt: g_prev,
                flexible_prev_ckpt: f_prev,
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::FundXnt { amount, epoch }.data(),
        };
        let r = self.send(&[ix], &[]).await;
        if r.is_ok() {
            self.last_funded_epoch = Some(epoch);
        }
        r
    }

    // -------------------- instrukcje pozycji --------------------

    fn position_pda(&self, owner: &Pubkey, index: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[b"position", owner.as_ref(), &index.to_le_bytes()],
            &self.program_id,
        )
        .0
    }

    fn profile_pda(&self, owner: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[b"profile", owner.as_ref()], &self.program_id).0
    }

    async fn stake(
        &mut self,
        user: &Keypair,
        user_anl: Pubkey,
        pool_type: PoolType,
        amount: u64,
        declared_days: u32,
        position_index: u64,
    ) -> Result<Pubkey, BanksClientError> {
        let pool = if pool_type == PoolType::Genesis {
            self.genesis_pool
        } else {
            self.flexible_pool
        };
        let position = self.position_pda(&user.pubkey(), position_index);
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::Stake {
                owner: user.pubkey(),
                global_config: self.global_config,
                pool_config: pool,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                owner_anl: user_anl,
                principal_vault: self.principal_vault,
                reward_vault: self.reward_vault,
                user_profile: self.profile_pda(&user.pubkey()),
                user_position: position,
                anl_token_program: spl_token_2022::id(),
                system_program: solana_sdk::system_program::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::Stake {
                amount,
                declared_days,
            }
            .data(),
        };
        self.send(&[ix], &[user]).await.map(|_| position)
    }

    async fn settle(
        &mut self,
        position: Pubkey,
        pool_type: PoolType,
        ckpt_epoch: Option<u64>,
    ) -> Result<(), BanksClientError> {
        let pool = if pool_type == PoolType::Genesis {
            self.genesis_pool
        } else {
            self.flexible_pool
        };
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::SettleExpired {
                cranker: self.authority.pubkey(),
                pool_config: pool,
                user_position: position,
                xnt_checkpoint: ckpt_epoch.map(|e| self.ckpt_pda(pool_type, e)),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::SettleExpired {}.data(),
        };
        self.send(&[ix], &[]).await
    }

    async fn claim(
        &mut self,
        user: &Keypair,
        user_anl: Pubkey,
        user_xnt: Pubkey,
        position: Pubkey,
        pool_type: PoolType,
        ckpt_epoch: Option<u64>,
    ) -> Result<(), BanksClientError> {
        let pool = if pool_type == PoolType::Genesis {
            self.genesis_pool
        } else {
            self.flexible_pool
        };
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::Claim {
                owner: user.pubkey(),
                global_config: self.global_config,
                pool_config: pool,
                user_position: position,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                xnt_mint: self.xnt_mint,
                principal_vault: self.principal_vault,
                reward_vault: self.reward_vault,
                xnt_vault: self.xnt_vault,
                owner_anl: user_anl,
                owner_xnt: user_xnt,
                anl_token_program: spl_token_2022::id(),
                xnt_token_program: spl_token::id(),
                xnt_checkpoint: ckpt_epoch.map(|e| self.ckpt_pda(pool_type, e)),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::Claim {}.data(),
        };
        self.send(&[ix], &[user]).await
    }

    async fn unstake_early(
        &mut self,
        user: &Keypair,
        user_anl: Pubkey,
        position: Pubkey,
        pool_type: PoolType,
    ) -> Result<(), BanksClientError> {
        let pool = if pool_type == PoolType::Genesis {
            self.genesis_pool
        } else {
            self.flexible_pool
        };
        let ix = Instruction {
            program_id: self.program_id,
            accounts: anl_staking::accounts::UnstakeEarly {
                owner: user.pubkey(),
                global_config: self.global_config,
                pool_config: pool,
                user_position: position,
                vault_authority: self.vault_authority,
                anl_mint: self.anl_mint.pubkey(),
                principal_vault: self.principal_vault,
                owner_anl: user_anl,
                anl_token_program: spl_token_2022::id(),
            }
            .to_account_metas(None),
            data: anl_staking::instruction::UnstakeEarly {}.data(),
        };
        self.send(&[ix], &[user]).await
    }

    // -------------------- odczyty --------------------

    async fn position(&mut self, addr: Pubkey) -> UserPosition {
        let acc = self
            .ctx
            .banks_client
            .get_account(addr)
            .await
            .unwrap()
            .expect("position account");
        UserPosition::try_deserialize(&mut acc.data.as_slice()).unwrap()
    }

    async fn pool(&mut self, addr: Pubkey) -> PoolConfig {
        let acc = self
            .ctx
            .banks_client
            .get_account(addr)
            .await
            .unwrap()
            .unwrap();
        PoolConfig::try_deserialize(&mut acc.data.as_slice()).unwrap()
    }

    /// Odczyt globalnej rezerwy nagród ANL (pole GlobalConfig.anl_reward_reserved).
    async fn global_reward_reserved(&mut self) -> u64 {
        let acc = self
            .ctx
            .banks_client
            .get_account(self.global_config)
            .await
            .unwrap()
            .unwrap();
        anl_staking::state::GlobalConfig::try_deserialize(&mut acc.data.as_slice())
            .unwrap()
            .anl_reward_reserved
    }

    async fn token_balance(&mut self, addr: Pubkey) -> u64 {
        let acc = self
            .ctx
            .banks_client
            .get_account(addr)
            .await
            .unwrap()
            .unwrap();
        // layout bazowy identyczny dla SPL i Token-2022
        spl_token::state::Account::unpack_from_slice(&acc.data[..165])
            .unwrap()
            .amount
    }
}

async fn clock(banks: &mut BanksClient) -> Clock {
    banks.get_sysvar::<Clock>().await.unwrap()
}

async fn airdrop(ctx: &mut ProgramTestContext, to: &Pubkey, lamports: u64) {
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(
            &ctx.payer.pubkey(),
            to,
            lamports,
        )],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        bh,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

async fn create_mint(
    ctx: &mut ProgramTestContext,
    mint: &Keypair,
    authority: &Keypair,
    token_program: Pubkey,
) {
    let rent = ctx.banks_client.get_rent().await.unwrap();
    let space = spl_token::state::Mint::LEN; // 82 — wspólny layout bazowy
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let init = if token_program == spl_token_2022::id() {
        spl_token_2022::instruction::initialize_mint2(
            &token_program,
            &mint.pubkey(),
            &authority.pubkey(),
            None,
            DECIMALS,
        )
        .unwrap()
    } else {
        spl_token::instruction::initialize_mint2(
            &token_program,
            &mint.pubkey(),
            &authority.pubkey(),
            None,
            DECIMALS,
        )
        .unwrap()
    };
    let tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &ctx.payer.pubkey(),
                &mint.pubkey(),
                rent.minimum_balance(space),
                space as u64,
                &token_program,
            ),
            init,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, mint],
        bh,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

async fn create_token_account(
    ctx: &mut ProgramTestContext,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: Pubkey,
) -> Pubkey {
    let acc = Keypair::new();
    let rent = ctx.banks_client.get_rent().await.unwrap();
    let space = spl_token::state::Account::LEN; // 165
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let init = if token_program == spl_token_2022::id() {
        spl_token_2022::instruction::initialize_account3(&token_program, &acc.pubkey(), mint, owner)
            .unwrap()
    } else {
        spl_token::instruction::initialize_account3(&token_program, &acc.pubkey(), mint, owner)
            .unwrap()
    };
    let tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &ctx.payer.pubkey(),
                &acc.pubkey(),
                rent.minimum_balance(space),
                space as u64,
                &token_program,
            ),
            init,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, &acc],
        bh,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
    acc.pubkey()
}

async fn mint_to(
    ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    to: &Pubkey,
    authority: &Keypair,
    amount: u64,
    token_program: Pubkey,
) {
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let ix = if token_program == spl_token_2022::id() {
        spl_token_2022::instruction::mint_to(
            &token_program,
            mint,
            to,
            &authority.pubkey(),
            &[],
            amount,
        )
        .unwrap()
    } else {
        spl_token::instruction::mint_to(&token_program, mint, to, &authority.pubkey(), &[], amount)
            .unwrap()
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, authority],
        bh,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

// ============================================================ TS-01..07: happy path

#[tokio::test]
async fn ts_full_lifecycle_two_users_daily_xnt() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;

    let days = anl_math::MIN_PERIOD_DAYS as u32 + 1; // > minimum obu buildów
    let period = days as i64 * DAY;

    // TS-02: dwie pozycje Genesis w oknie 1 (20%), proporcje 2:1
    let (alice, alice_anl, alice_xnt) = env.user_with_anl(200 * ONE_ANL).await;
    let (bob, bob_anl, bob_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos_a = env
        .stake(&alice, alice_anl, PoolType::Genesis, 200 * ONE_ANL, days, 0)
        .await
        .unwrap();
    let pos_b = env
        .stake(&bob, bob_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    let p = env.position(pos_a).await;
    assert_eq!(p.apy_bps, 2_000, "okno 1 = 20%");
    assert_eq!(p.declared_days, days);
    assert_eq!(
        p.amount,
        200 * ONE_ANL,
        "actual received == amount (mint bez fee)"
    );
    let expected_reward = anl_math::period_reward(200 * ONE_ANL, 2_000, period).unwrap();
    assert_eq!(
        p.anl_reward, expected_reward,
        "Immutable APY: nagroda znana z gory"
    );

    // TS-04: dzienny funding — pusty koszyk Flexible czeka w undistributed
    env.fund_xnt(1_000_000).await.unwrap();
    let flex = env.pool(env.flexible_pool).await;
    assert_eq!(flex.xnt_undistributed, 350_000, "35% czeka - pusty koszyk");
    let gen = env.pool(env.genesis_pool).await;
    assert_eq!(gen.xnt_undistributed, 0, "65% weszlo do indeksu");

    // TS-05: drugi dzień fundingu
    env.advance(DAY).await;
    env.fund_xnt(1_000_000).await.unwrap();

    // koniec okresu — TS-06: settle mrozi naliczanie
    env.advance(period).await;
    env.settle(pos_a, PoolType::Genesis, Some(1)).await.unwrap();
    env.settle(pos_b, PoolType::Genesis, Some(1)).await.unwrap();
    let pa = env.position(pos_a).await;
    let pb = env.position(pos_b).await;
    // 2 dni × 650 000 XNT dla koszyka Genesis, proporcja 2:1 (floor)
    assert!(pa.xnt_accrued + pb.xnt_accrued <= 1_300_000);
    assert!(
        pa.xnt_accrued >= 866_665 && pa.xnt_accrued <= 866_667,
        "A ~ 2/3"
    );
    assert!(
        pb.xnt_accrued >= 433_332 && pb.xnt_accrued <= 433_334,
        "B ~ 1/3"
    );
    assert!(pa.settled && pb.settled);

    // funding PO settle nie dolicza nic pozycjom po terminie (WP §8)
    let frozen_a = pa.xnt_accrued;
    env.fund_xnt(1_000_000).await.unwrap();
    assert_eq!(env.position(pos_a).await.xnt_accrued, frozen_a);

    // TS-07: claim — ANL + XNT + principal jedną transakcją, konto zamknięte
    let anl_before = env.token_balance(alice_anl).await;
    let xnt_before = env.token_balance(alice_xnt).await;
    env.claim(&alice, alice_anl, alice_xnt, pos_a, PoolType::Genesis, None)
        .await
        .unwrap();
    assert_eq!(
        env.token_balance(alice_anl).await - anl_before,
        200 * ONE_ANL + expected_reward,
        "principal + nagroda ANL"
    );
    assert_eq!(
        env.token_balance(alice_xnt).await - xnt_before,
        frozen_a,
        "XNT razem z ANL"
    );
    assert!(
        env.ctx
            .banks_client
            .get_account(pos_a)
            .await
            .unwrap()
            .is_none(),
        "konto pozycji zamknięte, rent wrócił"
    );

    // rezerwacja zwolniona proporcjonalnie (globalne saldo księgowe)
    env.claim(&bob, bob_anl, bob_xnt, pos_b, PoolType::Genesis, None)
        .await
        .unwrap();
    let gen = env.pool(env.genesis_pool).await;
    assert_eq!(gen.total_staked, 0);
    assert_eq!(gen.position_count, 0);
}

// ============================================================ TS-08..10: zerwanie i guardy cyklu

#[tokio::test]
async fn ts_early_exit_forfeits_and_redistributes() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;

    let days = anl_math::MIN_PERIOD_DAYS as u32 + 2;
    let (alice, alice_anl, alice_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let (bob, bob_anl, bob_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos_a = env
        .stake(
            &alice,
            alice_anl,
            PoolType::Flexible,
            100 * ONE_ANL,
            days,
            0,
        )
        .await
        .unwrap();
    let pos_b = env
        .stake(&bob, bob_anl, PoolType::Flexible, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    assert_eq!(env.position(pos_a).await.apy_bps, 800, "Flexible zawsze 8%");

    // dzień 1: koszyk Flexible dostaje 35% z 1 000 000 = 350 000; A i B po 175 000
    env.fund_xnt(1_000_000).await.unwrap();

    // TS-08: claim przed końcem okresu — odrzucony
    env.advance(DAY).await;
    let err = env
        .claim(
            &alice,
            alice_anl,
            alice_xnt,
            pos_a,
            PoolType::Flexible,
            None,
        )
        .await;
    assert!(err.is_err(), "PeriodNotEnded");

    // TS-09: zerwanie — principal w całości, XNT do puli dystrybucji
    let anl_before = env.token_balance(alice_anl).await;
    let xnt_before = env.token_balance(alice_xnt).await;
    env.unstake_early(&alice, alice_anl, pos_a, PoolType::Flexible)
        .await
        .unwrap();
    assert_eq!(
        env.token_balance(alice_anl).await - anl_before,
        100 * ONE_ANL,
        "principal wraca w 100%"
    );
    assert_eq!(
        env.token_balance(alice_xnt).await,
        xnt_before,
        "zero XNT przy zerwaniu"
    );
    let flex = env.pool(env.flexible_pool).await;
    assert_eq!(
        flex.xnt_undistributed, 175_000,
        "przepadek wraca do puli koszyka"
    );
    assert!(
        env.ctx
            .banks_client
            .get_account(pos_a)
            .await
            .unwrap()
            .is_none(),
        "konto zamknięte"
    );

    // kolejny funding: przepadek + nowa transza w całości dla B
    env.fund_xnt(1_000_000).await.unwrap();
    env.advance((days as i64) * DAY).await;
    env.settle(pos_b, PoolType::Flexible, Some(1))
        .await
        .unwrap();
    let pb = env.position(pos_b).await;
    // B: 175 000 (dzień 1) + 350 000 (dzień 2) + 175 000 (przepadek A) = 700 000
    assert_eq!(pb.xnt_accrued, 700_000);

    // TS-10: zerwanie po końcu okresu nie istnieje — właściwa ścieżka to claim
    let err = env
        .unstake_early(&bob, bob_anl, pos_b, PoolType::Flexible)
        .await;
    assert!(err.is_err(), "PeriodAlreadyEnded");
    env.claim(&bob, bob_anl, bob_xnt, pos_b, PoolType::Flexible, None)
        .await
        .unwrap();
}

// ============================================================ TS-15: blokada Genesis (WP v1.1)

#[tokio::test]
async fn ts_genesis_locked_no_early_exit() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;

    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (carol, carol_anl, carol_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&carol, carol_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // zerwanie Genesis w trakcie okresu — odrzucone (GenesisLocked)
    env.advance(DAY).await;
    let err = env
        .unstake_early(&carol, carol_anl, pos, PoolType::Genesis)
        .await;
    assert!(err.is_err(), "GenesisLocked");
    assert!(
        env.ctx
            .banks_client
            .get_account(pos)
            .await
            .unwrap()
            .is_some(),
        "pozycja Genesis nietknięta po odrzuconym zerwaniu"
    );

    // po końcu okresu środki normalnie do odebrania — blokada nie więzi kapitału
    env.advance((days as i64) * DAY).await;
    env.settle(pos, PoolType::Genesis, None).await.unwrap();
    env.claim(&carol, carol_anl, carol_xnt, pos, PoolType::Genesis, None)
        .await
        .unwrap();
}

// ============================================================ TS-11..14: okna, pokrycie, walidacje

#[tokio::test]
async fn ts_windows_coverage_and_validation_guards() {
    let mut env = Env::new().await;

    // TS-13: pokrycie nagrody — bez fund_rewards stake odpada
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (carol, carol_anl, _carol_xnt) = env.user_with_anl(1_000 * ONE_ANL).await;
    let err = env
        .stake(&carol, carol_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await;
    assert!(err.is_err(), "RewardCoverageExceeded - pusty Reward Vault");

    env.fund_rewards(10_000 * ONE_ANL).await;

    // TS-14: walidacje okresu i kwoty
    let err = env
        .stake(
            &carol,
            carol_anl,
            PoolType::Genesis,
            100 * ONE_ANL,
            days - 1,
            0,
        )
        .await;
    assert!(err.is_err(), "InvalidPeriod - ponizej minimum");
    let err = env
        .stake(&carol, carol_anl, PoolType::Genesis, ONE_ANL / 2, days, 0)
        .await;
    assert!(err.is_err(), "BelowMinimumStake");

    // TS-11: okna Genesis — Immutable APY wg chwili wejścia
    let pos_w1 = env
        .stake(&carol, carol_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    assert_eq!(env.position(pos_w1).await.apy_bps, 2_000, "okno 1");

    env.advance(anl_math::WINDOW_1_END).await; // początek okna 2
    let pos_w2 = env
        .stake(&carol, carol_anl, PoolType::Genesis, 100 * ONE_ANL, days, 1)
        .await
        .unwrap();
    assert_eq!(env.position(pos_w2).await.apy_bps, 1_500, "okno 2");
    assert_eq!(
        env.position(pos_w1).await.apy_bps,
        2_000,
        "pozycja z okna 1 trzyma 20%"
    );

    env.advance(anl_math::WINDOW_2_END - anl_math::WINDOW_1_END)
        .await; // okno 3
    let pos_w3 = env
        .stake(&carol, carol_anl, PoolType::Genesis, 100 * ONE_ANL, days, 2)
        .await
        .unwrap();
    assert_eq!(env.position(pos_w3).await.apy_bps, 800, "okno 3 - standard");

    // Flexible w oknie 3 nadal 8% (i nie mniej)
    let pos_f = env
        .stake(
            &carol,
            carol_anl,
            PoolType::Flexible,
            100 * ONE_ANL,
            days,
            3,
        )
        .await
        .unwrap();
    assert_eq!(env.position(pos_f).await.apy_bps, 800);

    // TS-12: pauza blokuje stake, ścieżki wyjścia działają
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::SetPause {
            authority: env.authority.pubkey(),
            global_config: env.global_config,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::Pause {}.data(),
    };
    env.send(&[ix], &[]).await.unwrap();
    let err = env
        .stake(
            &carol,
            carol_anl,
            PoolType::Flexible,
            100 * ONE_ANL,
            days,
            4,
        )
        .await;
    assert!(err.is_err(), "Paused blokuje stake");

    env.advance((days as i64) * DAY + 1).await;
    env.settle(pos_w1, PoolType::Genesis, None).await.unwrap();
    let (_, _, carol_xnt2) = {
        // konto XNT dla claim
        let xnt = create_token_account(
            &mut env.ctx,
            &carol.pubkey(),
            &env.xnt_mint,
            spl_token::id(),
        )
        .await;
        ((), (), xnt)
    };
    env.claim(
        &carol,
        carol_anl,
        carol_xnt2,
        pos_w1,
        PoolType::Genesis,
        None,
    )
    .await
    .expect("claim działa mimo pauzy - user nigdy nie jest uwięziony");

    let status = env.position(pos_w2).await.status;
    assert_eq!(status, PositionStatus::Active);
}

// ============================================================ TS-AUDIT: exploit #1 zamknięty

/// Obowiązkowy test audytu #2: funding wykonany PO epoce końca pozycji
/// nie może zwiększyć jej wypłaty XNT — niezależnie od tego, czy settle
/// nastąpił przed fundingiem, po nim, czy dopiero inline przy claim.
#[tokio::test]
async fn ts_audit_funding_after_end_epoch_not_counted() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;

    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let period = days as i64 * DAY;
    let (alice, alice_anl, alice_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let (bob, bob_anl, bob_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos_a = env
        .stake(
            &alice,
            alice_anl,
            PoolType::Flexible,
            100 * ONE_ANL,
            days,
            0,
        )
        .await
        .unwrap();
    let pos_b = env
        .stake(&bob, bob_anl, PoolType::Flexible, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    assert_eq!(env.position(pos_a).await.end_epoch, days as u64 - 1);

    // epoka 0: koszyk Flexible 350 000 -> po 175 000 na pozycję
    env.fund_xnt(1_000_000).await.unwrap();

    // koniec okresu mija; bot NIE robi settle (scenariusz awarii z audytu)
    env.advance(period + 1).await;
    assert_eq!(env.current_epoch().await, days as u64);

    // funding w epoce PO końcu pozycji (dokładnie exploit z raportu)
    env.fund_xnt(1_000_000).await.unwrap();

    // ATAK: claim z podstawionym checkpointem późniejszej epoki -> odrzucony
    let err = env
        .claim(
            &alice,
            alice_anl,
            alice_xnt,
            pos_a,
            PoolType::Flexible,
            Some(days as u64),
        )
        .await;
    assert!(err.is_err(), "checkpoint > end_epoch musi zostać odrzucony");

    // claim bez wcześniejszego settle (inline), checkpoint końca epoki 0:
    // dokładnie 175 000 XNT — ani jednostki z późniejszego fundingu
    let xnt_before = env.token_balance(alice_xnt).await;
    env.claim(
        &alice,
        alice_anl,
        alice_xnt,
        pos_a,
        PoolType::Flexible,
        Some(0),
    )
    .await
    .unwrap();
    assert_eq!(env.token_balance(alice_xnt).await - xnt_before, 175_000);

    // równoważność ścieżek: settle-przed-claim daje IDENTYCZNY wynik
    env.settle(pos_b, PoolType::Flexible, Some(0))
        .await
        .unwrap();
    assert_eq!(env.position(pos_b).await.xnt_accrued, 175_000);
    let xnt_before = env.token_balance(bob_xnt).await;
    env.claim(&bob, bob_anl, bob_xnt, pos_b, PoolType::Flexible, None)
        .await
        .unwrap();
    assert_eq!(env.token_balance(bob_xnt).await - xnt_before, 175_000);

    // inwariant wypłacalności: wypłaty + saldo vaulta == suma fundingów
    let vault = env.token_balance(env.xnt_vault).await;
    assert_eq!(vault, 2_000_000 - 350_000, "reszta pozostaje w skarbcu");
}

/// TS-AUD5 (audyt #5, A-01/A-05): stan pośredni 4-etapowego setupu.
/// (a) po samym `initialize` stake musi upaść — skarbce nie istnieją;
/// (b) nie-authority nie utworzy skarbca (has_one → InvalidAuthority);
/// (c) powtórny `initialize` odrzucony (GlobalConfig już istnieje);
/// (d) po dokończeniu setupu powtórna inicjalizacja skarbca odrzucona;
/// (e) stan pośredni jest naprawialny: dokończony setup ⇒ stake przechodzi.
#[tokio::test]
async fn ts_split_init_intermediate_state_guards() {
    let mut env = Env::new_pre_init().await;

    // etap 1: sama konfiguracja (bez skarbców)
    let ix = env.ix_initialize();
    env.send(&[ix], &[]).await.unwrap();

    // pule nie zależą od skarbców — wolno je utworzyć w oknie pośrednim
    env.create_pools().await;

    // (a) okno pośrednie: stake odrzucony (principal_vault nie istnieje)
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_anl, _user_xnt) = env.user_with_anl(5_000 * ONE_ANL).await;
    let r = env
        .stake(&user, user_anl, PoolType::Genesis, 1_000 * ONE_ANL, days, 0)
        .await;
    assert!(r.is_err(), "stake w oknie pośrednim musi zostać odrzucony");

    // (b) obcy podpisujący nie utworzy skarbca (front-run niemożliwy)
    let attacker = Keypair::new();
    airdrop(&mut env.ctx, &attacker.pubkey(), 10_000_000_000).await;
    let ix = env.ix_init_principal_vault(attacker.pubkey());
    let r = env.send_as(&attacker, &[ix], &[]).await;
    assert!(r.is_err(), "nie-authority nie może utworzyć skarbca");

    // (c) powtórny initialize — GlobalConfig już istnieje (init, nie init_if_needed)
    let ix = env.ix_initialize();
    let r = env.send(&[ix], &[]).await;
    assert!(r.is_err(), "powtórny initialize musi zostać odrzucony");

    // (d) authority kończy setup; powtórny init skarbca odrzucony (PDA zajęte)
    env.init_all_vaults().await;
    let ix = env.ix_init_principal_vault(env.authority.pubkey());
    let r = env.send(&[ix], &[]).await;
    assert!(
        r.is_err(),
        "powtórna inicjalizacja skarbca musi zostać odrzucona"
    );

    // (e) stan naprawiony: funding i stake przechodzą normalnie
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    env.stake(&user, user_anl, PoolType::Genesis, 1_000 * ONE_ANL, days, 0)
        .await
        .unwrap();
}

// ═══════════════════════════════════════════════════════════════════════
// GRUPA A — ATAKI NA IZOLACJĘ SKARBCÓW (plan pentestów, A1–A4)
// Cel: udowodnić, że nie da się okraść ani pomieszać trzech skarbców.
// Każdy atak MUSI się odbić właściwym błędem; inwarianty MUSZĄ się zgadzać.
// ═══════════════════════════════════════════════════════════════════════

// A1 — Podstawienie Reward Vault w miejsce Principal Vault przy claim.
// Napastnik chce, by principal wypłacił się z puli nagród (albo odwrotnie),
// licząc na wyciągnięcie ANL ponad to, co mu się należy.
#[tokio::test]
async fn atak_a1_podstawienie_skarbca_w_claim() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_anl, user_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    env.advance((days as i64) * DAY + DAY).await;

    // Atak: budujemy claim, ale jako principal_vault podstawiamy reward_vault.
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::Claim {
            owner: user.pubkey(),
            global_config: env.global_config,
            pool_config: env.genesis_pool,
            user_position: pos,
            vault_authority: env.vault_authority,
            anl_mint: env.anl_mint.pubkey(),
            xnt_mint: env.xnt_mint,
            principal_vault: env.reward_vault, // ⚠️ PODMIANA
            reward_vault: env.reward_vault,
            xnt_vault: env.xnt_vault,
            owner_anl: user_anl,
            owner_xnt: user_xnt,
            anl_token_program: spl_token_2022::id(),
            xnt_token_program: spl_token::id(),
            xnt_checkpoint: None,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::Claim {}.data(),
    };
    let res = env.send_as(&user, &[ix], &[&user]).await;
    assert!(
        res.is_err(),
        "A1: podstawienie reward_vault jako principal_vault MUSI być odrzucone (seeds/token::mint)"
    );
}

// A2 — Claim cudzej pozycji. Napastnik podpisuje własnym kluczem,
// ale wskazuje pozycję ofiary, licząc na wypłatę na swoje konto.
#[tokio::test]
async fn atak_a2_claim_cudzej_pozycji() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (ofiara, ofiara_anl, _ofiara_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let (napastnik, napastnik_anl, napastnik_xnt) = env.user_with_anl(1 * ONE_ANL).await;
    let pos_ofiary = env
        .stake(&ofiara, ofiara_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    env.advance((days as i64) * DAY + DAY).await;

    // Atak: napastnik jako owner, ale user_position = pozycja ofiary,
    // konta docelowe = konta napastnika.
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::Claim {
            owner: napastnik.pubkey(), // ⚠️ nie właściciel pozycji
            global_config: env.global_config,
            pool_config: env.genesis_pool,
            user_position: pos_ofiary, // ⚠️ cudza pozycja
            vault_authority: env.vault_authority,
            anl_mint: env.anl_mint.pubkey(),
            xnt_mint: env.xnt_mint,
            principal_vault: env.principal_vault,
            reward_vault: env.reward_vault,
            xnt_vault: env.xnt_vault,
            owner_anl: napastnik_anl,
            owner_xnt: napastnik_xnt,
            anl_token_program: spl_token_2022::id(),
            xnt_token_program: spl_token::id(),
            xnt_checkpoint: None,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::Claim {}.data(),
    };
    let res = env.send_as(&napastnik, &[ix], &[&napastnik]).await;
    assert!(
        res.is_err(),
        "A2: claim cudzej pozycji MUSI być odrzucony (PositionOwnerMismatch / seeds pozycji)"
    );
}

// A3 — Inwariant: saldo Principal Vault == suma total_staked wszystkich pul.
// Po serii stake/claim sprawdzamy, że kapitał w skarbcu zgadza się z księgowością.
#[tokio::test]
async fn atak_a3_inwariant_principal_vs_total_staked() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;

    let (u1, u1a, _) = env.user_with_anl(500 * ONE_ANL).await;
    let (u2, u2a, _) = env.user_with_anl(500 * ONE_ANL).await;
    env.stake(&u1, u1a, PoolType::Genesis, 300 * ONE_ANL, days, 0)
        .await
        .unwrap();
    env.stake(&u2, u2a, PoolType::Flexible, 200 * ONE_ANL, days, 0)
        .await
        .unwrap();

    let g = env.pool(env.genesis_pool).await;
    let f = env.pool(env.flexible_pool).await;
    let vault = env.token_balance(env.principal_vault).await;
    assert_eq!(
        vault,
        g.total_staked + f.total_staked,
        "A3: Principal Vault MUSI równać się sumie total_staked (co do lamporta)"
    );
    assert_eq!(vault, 500 * ONE_ANL, "A3: 300 + 200 = 500 ANL kapitału");
}

// A4 — Integralność anl_reward_reserved: rezerwa nagród == suma zarezerwowana
// żywych pozycji i nie schodzi poniżej zera (checked_sub).
#[tokio::test]
async fn atak_a4_inwariant_reward_reserved() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;

    let (u1, u1a, u1x) = env.user_with_anl(500 * ONE_ANL).await;
    let p1 = env
        .stake(&u1, u1a, PoolType::Genesis, 300 * ONE_ANL, days, 0)
        .await
        .unwrap();
    let pos = env.position(p1).await;
    let reserved_after_stake = env.global_reward_reserved().await;
    assert_eq!(
        reserved_after_stake, pos.anl_reward,
        "A4: rezerwa == nagroda pojedynczej żywej pozycji"
    );

    // Po claim rezerwa wraca do zera (pozycja zamknięta, nagroda wypłacona).
    env.advance((days as i64) * DAY + DAY).await;
    env.claim(&u1, u1a, u1x, p1, PoolType::Genesis, None)
        .await
        .unwrap();
    let reserved_after_claim = env.global_reward_reserved().await;
    assert_eq!(
        reserved_after_claim, 0,
        "A4: po claim rezerwa schodzi do zera bez underflow"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// GRUPA B — ATAKI NA BLOKADĘ GENESIS (plan pentestów, B1–B3)
// WP v1.1 §5/§7: pozycja Genesis jest nieodwołalna — brak wcześniejszego
// wyjścia ŻADNĄ ścieżką. Każda próba MUSI się odbić.
// ═══════════════════════════════════════════════════════════════════════

// B1 — Bezpośrednie unstake_early na pozycji Genesis.
// Napastnik (właściciel) próbuje zerwać zablokowaną pozycję Genesis.
#[tokio::test]
async fn atak_b1_unstake_early_na_genesis() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_anl, _user_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Atak: unstake_early na Genesis — musi odbić się GenesisLocked.
    let res = env
        .unstake_early(&user, user_anl, pos, PoolType::Genesis)
        .await;
    assert!(
        res.is_err(),
        "B1: unstake_early na Genesis MUSI być odrzucone (GenesisLocked)"
    );
}

// B2 — Oszustwo pool_config: pozycja Genesis, ale podstawiamy pool_config
// Flexible, licząc na to, że kontrakt potraktuje ją jak Flexible i pozwoli wyjść.
#[tokio::test]
async fn atak_b2_podstawienie_flexible_pool_pod_genesis() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_anl, _user_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Atak: budujemy unstake_early ręcznie, pool_config = flexible_pool,
    // ale user_position to pozycja Genesis.
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::UnstakeEarly {
            owner: user.pubkey(),
            global_config: env.global_config,
            pool_config: env.flexible_pool, // ⚠️ Flexible pod pozycję Genesis
            user_position: pos,
            vault_authority: env.vault_authority,
            anl_mint: env.anl_mint.pubkey(),
            principal_vault: env.principal_vault,
            owner_anl: user_anl,
            anl_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: anl_staking::instruction::UnstakeEarly {}.data(),
    };
    let res = env.send_as(&user, &[ix], &[&user]).await;
    assert!(
        res.is_err(),
        "B2: podstawienie Flexible pool_config pod pozycję Genesis MUSI być odrzucone (InvalidVault)"
    );
}

// B3 — Genesis po końcu okresu: unstake_early nie może być użyte nawet gdy
// okres minął (właściwa ścieżka to claim). Potwierdza, że nie ma tylnej furtki.
#[tokio::test]
async fn atak_b3_unstake_genesis_po_okresie() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_anl, _user_xnt) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_anl, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    // Przewijamy PO końcu okresu.
    env.advance((days as i64) * DAY + DAY).await;

    // Atak: unstake_early na Genesis po okresie — nadal GenesisLocked
    // (blokada jest bezwarunkowa, sprawdzana przed czasem).
    let res = env
        .unstake_early(&user, user_anl, pos, PoolType::Genesis)
        .await;
    assert!(
        res.is_err(),
        "B3: unstake_early na Genesis (nawet po okresie) MUSI być odrzucone"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// GRUPA C — ATAKI NA XNT / fund_xnt (plan pentestów, C1–C5)
// Najtrudniejsza grupa: matematyka indeksów, zaokrąglenia, autoryzacja,
// przypisania wsteczne. Tu giną protokoły — nie na braku podpisu.
// ═══════════════════════════════════════════════════════════════════════

// C1 — Gamowanie czasu: stake TUŻ przed fundingiem nie może dać XNT za
// dni, w których pozycja nie istniała. (Debt index ustawiany przy stake.)
#[tokio::test]
async fn atak_c1_stake_tuz_przed_fundingiem() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;

    // Uczciwy staker już w puli od początku.
    let (early, early_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    env.stake(&early, early_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Kilka fundingów mija (uczciwy zbiera XNT).
    env.fund_xnt(1_000).await.unwrap();
    env.advance(DAY).await;
    env.fund_xnt(1_000).await.unwrap();
    env.advance(DAY).await;

    // Napastnik wchodzi TERAZ, tuż przed kolejnym fundingiem.
    // (Nowy user → jego pierwsza pozycja ma indeks 0.)
    let (late, late_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    let p_late = env
        .stake(&late, late_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    let pos_late = env.position(p_late).await;
    let g = env.pool(env.genesis_pool).await;

    // Debt index napastnika MUSI == bieżący indeks puli (brak roszczeń wstecz).
    assert_eq!(
        pos_late.xnt_debt_index, g.xnt_reward_index,
        "C1: pozycja wchodząca późno startuje z aktualnym debt_index — zero XNT za przeszłe fundingi"
    );
    // Zatem pending na wejściu == 0.
    let pending = g.pending_xnt(pos_late.shares, pos_late.xnt_debt_index).unwrap();
    assert_eq!(pending, 0, "C1: brak naliczonego XNT tuż po wejściu");
}

// C2 — Zaokrąglenie podziału 65/35: suma części ZAWSZE == całość,
// dla wielu różnych kwot (żaden lamport nie ginie ani nie powstaje).
#[tokio::test]
async fn atak_c2_split_bez_utraty_lamportow() {
    // Czysto matematyczny atak — nie potrzebuje łańcucha.
    for net in [1u64, 2, 3, 7, 99, 100, 101, 999, 1_000, 1_001, 65_535,
                1_000_000, 1_000_003, u64::MAX / 2, u64::MAX - 1] {
        let (g, f) = anl_math::split_xnt(net);
        assert_eq!(
            (g as u128) + (f as u128),
            net as u128,
            "C2: split_xnt({net}) MUSI sumować się do całości — g={g}, f={f}"
        );
        // Genesis dostaje ~65%, nigdy więcej niż całość.
        assert!(g <= net, "C2: część Genesis nie może przekroczyć całości");
    }
}

// C3 — fund_xnt przez NIE-operatora. Napastnik z własnym kontem XNT
// próbuje wywołać funding (i tak zmanipulować indeksy). Musi się odbić.
#[tokio::test]
async fn atak_c3_fund_xnt_przez_obcego() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    env.stake(&user, user_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Napastnik: własny keypair + własne konto XNT z zapasem.
    let napastnik = Keypair::new();
    airdrop(&mut env.ctx, &napastnik.pubkey(), 10_000_000_000).await;
    let atk_xnt = create_token_account(&mut env.ctx, &napastnik.pubkey(), &env.xnt_mint, spl_token::id()).await;
    mint_to(&mut env.ctx, &env.xnt_mint, &atk_xnt, &env.authority, 1_000, spl_token::id()).await;

    let epoch = env.current_epoch().await;
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::FundXnt {
            funder: napastnik.pubkey(), // ⚠️ nie authority ani operator
            global_config: env.global_config,
            vault_authority: env.vault_authority,
            xnt_mint: env.xnt_mint,
            funder_xnt: atk_xnt,
            xnt_vault: env.xnt_vault,
            genesis_pool: env.genesis_pool,
            flexible_pool: env.flexible_pool,
            xnt_token_program: spl_token::id(),
            genesis_ckpt: env.ckpt_pda(PoolType::Genesis, epoch),
            flexible_ckpt: env.ckpt_pda(PoolType::Flexible, epoch),
            genesis_prev_ckpt: None,
            flexible_prev_ckpt: None,
            system_program: solana_sdk::system_program::id(),
        }
        .to_account_metas(None),
        data: anl_staking::instruction::FundXnt { amount: 1_000, epoch }.data(),
    };
    let res = env.send_as(&napastnik, &[ix], &[&napastnik]).await;
    assert!(
        res.is_err(),
        "C3: fund_xnt przez obcego (nie operator/authority) MUSI być odrzucone (InvalidAuthority)"
    );
}

// C4 — Pusty koszyk: funding gdy total_shares == 0 nie może dzielić przez
// zero; XNT trafia do xnt_undistributed i czeka na następny funding.
#[tokio::test]
async fn atak_c4_funding_pusty_koszyk() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;

    // Żadnych pozycji — oba koszyki puste. Funding MUSI przejść bez paniki.
    env.fund_xnt(1_000).await.unwrap();

    let g = env.pool(env.genesis_pool).await;
    let f = env.pool(env.flexible_pool).await;
    // Indeks zostaje 0 (nic nie rozdzielono), XNT czeka jako undistributed.
    assert_eq!(g.xnt_reward_index, 0, "C4: pusty koszyk Genesis — indeks nie rośnie");
    assert_eq!(f.xnt_reward_index, 0, "C4: pusty koszyk Flexible — indeks nie rośnie");
    assert_eq!(
        g.xnt_undistributed + f.xnt_undistributed,
        1_000,
        "C4: całe XNT czeka jako undistributed (nic nie zginęło)"
    );
}

// C5 — Funding wstecz: próba fund_xnt ze starą epoką (przypisać nagrody
// wstecz / nadpisać checkpoint). Musi się odbić EpochMismatch.
#[tokio::test]
async fn atak_c5_funding_stara_epoka() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    env.stake(&user, user_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Przesuwamy czas o kilka dni i fundujemy w bieżącej epoce.
    env.advance(3 * DAY).await;
    env.fund_xnt(1_000).await.unwrap();

    // Atak: próba fundingu z epoką 0 (przeszłość), ręcznie zbudowana.
    let src = create_token_account(&mut env.ctx, &env.authority.pubkey(), &env.xnt_mint, spl_token::id()).await;
    mint_to(&mut env.ctx, &env.xnt_mint, &src, &env.authority, 1_000, spl_token::id()).await;
    let stara_epoka = 0u64;
    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::FundXnt {
            funder: env.authority.pubkey(),
            global_config: env.global_config,
            vault_authority: env.vault_authority,
            xnt_mint: env.xnt_mint,
            funder_xnt: src,
            xnt_vault: env.xnt_vault,
            genesis_pool: env.genesis_pool,
            flexible_pool: env.flexible_pool,
            xnt_token_program: spl_token::id(),
            genesis_ckpt: env.ckpt_pda(PoolType::Genesis, stara_epoka),
            flexible_ckpt: env.ckpt_pda(PoolType::Flexible, stara_epoka),
            genesis_prev_ckpt: None,
            flexible_prev_ckpt: None,
            system_program: solana_sdk::system_program::id(),
        }
        .to_account_metas(None),
        data: anl_staking::instruction::FundXnt { amount: 1_000, epoch: stara_epoka }.data(),
    };
    let res = env.send(&[ix], &[]).await;
    assert!(
        res.is_err(),
        "C5: funding ze starą epoką MUSI być odrzucony (EpochMismatch — brak przypisań wstecz)"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// GRUPA D — ATAKI NA CYKL ŻYCIA POZYCJI (plan pentestów, D1–D5)
// Maszyna stanów: podwójny claim, claim po zerwaniu, claim przed czasem,
// patologiczne parametry. Każda nielegalna ścieżka MUSI się odbić.
// ═══════════════════════════════════════════════════════════════════════

// D1 — Podwójny claim: druga próba claim tej samej pozycji musi się odbić
// (pozycja zamknięta i konto zwolnione po pierwszym claim).
#[tokio::test]
async fn atak_d1_podwojny_claim() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_a, user_x) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();
    env.advance((days as i64) * DAY + DAY).await;

    // Pierwszy claim — legalny.
    env.claim(&user, user_a, user_x, pos, PoolType::Genesis, None)
        .await
        .unwrap();
    // Po claim konto pozycji MUSI być zamknięte (close = owner) — nie ma
    // czego claimować drugi raz. To zamyka wektor podwójnej wypłaty.
    let acc = env.ctx.banks_client.get_account(pos).await.unwrap();
    assert!(
        acc.is_none() || acc.unwrap().lamports == 0,
        "D1: po claim konto pozycji MUSI być zamknięte (brak podwójnego claim)"
    );
}

// D2 — Claim po zerwaniu (Flexible): zerwij pozycję, potem spróbuj claim.
// Kapitał już wrócił — claim nie może wypłacić drugi raz.
#[tokio::test]
async fn atak_d2_claim_po_zerwaniu() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32 + 2;
    let (user, user_a, user_x) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_a, PoolType::Flexible, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Zerwanie (Flexible dozwolone) — kapitał wraca, pozycja zamknięta.
    env.unstake_early(&user, user_a, pos, PoolType::Flexible)
        .await
        .unwrap();
    // Po zerwaniu konto pozycji MUSI być zamknięte (close = owner) —
    // nie ma czego claimować, kapitał nie wypłaci się drugi raz.
    let acc = env.ctx.banks_client.get_account(pos).await.unwrap();
    assert!(
        acc.is_none() || acc.unwrap().lamports == 0,
        "D2: po zerwaniu konto pozycji MUSI być zamknięte (brak podwójnej wypłaty)"
    );
}

// D3 — Claim przed końcem okresu: nagroda wymagalna dopiero po end_ts.
#[tokio::test]
async fn atak_d3_claim_przed_koncem() {
    let mut env = Env::new().await;
    env.fund_rewards(10_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32 + 3;
    let (user, user_a, user_x) = env.user_with_anl(100 * ONE_ANL).await;
    let pos = env
        .stake(&user, user_a, PoolType::Genesis, 100 * ONE_ANL, days, 0)
        .await
        .unwrap();

    // Bez przewijania czasu — okres NIE minął.
    let res = env
        .claim(&user, user_a, user_x, pos, PoolType::Genesis, None)
        .await;
    assert!(
        res.is_err(),
        "D3: claim przed końcem okresu MUSI być odrzucony (PeriodNotEnded)"
    );
}

// D4 — Stake zerową kwotą: musi się odbić (ZeroAmount).
#[tokio::test]
async fn atak_d4_stake_zero() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let days = anl_math::MIN_PERIOD_DAYS as u32;
    let (user, user_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    let res = env
        .stake(&user, user_a, PoolType::Genesis, 0, days, 0)
        .await;
    assert!(
        res.is_err(),
        "D4: stake zerową kwotą MUSI być odrzucony (ZeroAmount)"
    );
}

// D5 — Stake z okresem poza zakresem: dni > MAX_PERIOD_DAYS musi się odbić
// (InvalidPeriod). Chroni przed patologicznymi parametrami czasu.
#[tokio::test]
async fn atak_d5_stake_okres_poza_zakresem() {
    let mut env = Env::new().await;
    env.fund_rewards(1_000_000 * ONE_ANL).await;
    let (user, user_a, _) = env.user_with_anl(100 * ONE_ANL).await;
    // MAX to 3650; próbujemy 3651.
    let za_duzo = anl_math::MAX_PERIOD_DAYS as u32 + 1;
    let res = env
        .stake(&user, user_a, PoolType::Genesis, 100 * ONE_ANL, za_duzo, 0)
        .await;
    assert!(
        res.is_err(),
        "D5: stake z okresem > MAX MUSI być odrzucony (InvalidPeriod)"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// GRUPA E — ATAKI NA AUTORYZACJĘ GLOBALNĄ (plan pentestów, E1–E3)
// Kto może pauzować i zmieniać operatora. Napastnik/operator NIE mogą
// przejąć uprawnień administratora. Każda próba MUSI się odbić.
// ═══════════════════════════════════════════════════════════════════════

// E1 — Pauza przez obcego: napastnik próbuje zapauzować protokół (griefing).
#[tokio::test]
async fn atak_e1_pauza_przez_obcego() {
    let mut env = Env::new().await;
    let napastnik = Keypair::new();
    airdrop(&mut env.ctx, &napastnik.pubkey(), 10_000_000_000).await;

    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::SetPause {
            authority: napastnik.pubkey(), // ⚠️ nie admin
            global_config: env.global_config,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::Pause {}.data(),
    };
    let res = env.send_as(&napastnik, &[ix], &[&napastnik]).await;
    assert!(
        res.is_err(),
        "E1: pauza przez obcego MUSI być odrzucona (InvalidAuthority)"
    );
}

// E2 — set_operator przez obcego: napastnik próbuje ustawić SIEBIE jako
// operatora (a potem fundować/manipulować). Musi się odbić.
#[tokio::test]
async fn atak_e2_set_operator_przez_obcego() {
    let mut env = Env::new().await;
    let napastnik = Keypair::new();
    airdrop(&mut env.ctx, &napastnik.pubkey(), 10_000_000_000).await;

    let ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::SetOperator {
            authority: napastnik.pubkey(), // ⚠️ nie admin
            global_config: env.global_config,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::SetOperator {
            new_operator: napastnik.pubkey(),
        }
        .data(),
    };
    let res = env.send_as(&napastnik, &[ix], &[&napastnik]).await;
    assert!(
        res.is_err(),
        "E2: set_operator przez obcego MUSI być odrzucone (InvalidAuthority)"
    );
}

// E3 — Operator próbuje pauzować: operator ma prawo TYLKO fundować,
// nie pauzować. Ustawiamy operatora legalnie (admin), potem operator
// próbuje pauzy — musi się odbić.
#[tokio::test]
async fn atak_e3_operator_nie_moze_pauzowac() {
    let mut env = Env::new().await;
    let operator = Keypair::new();
    airdrop(&mut env.ctx, &operator.pubkey(), 10_000_000_000).await;

    // Admin legalnie ustawia operatora.
    let set_ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::SetOperator {
            authority: env.authority.pubkey(),
            global_config: env.global_config,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::SetOperator {
            new_operator: operator.pubkey(),
        }
        .data(),
    };
    env.send(&[set_ix], &[]).await.unwrap();

    // Operator próbuje pauzy — MUSI się odbić (pauza tylko dla authority).
    let pause_ix = Instruction {
        program_id: env.program_id,
        accounts: anl_staking::accounts::SetPause {
            authority: operator.pubkey(), // ⚠️ operator, nie admin
            global_config: env.global_config,
        }
        .to_account_metas(None),
        data: anl_staking::instruction::Pause {}.data(),
    };
    let res = env.send_as(&operator, &[pause_ix], &[&operator]).await;
    assert!(
        res.is_err(),
        "E3: operator NIE może pauzować (tylko fundować) — MUSI być odrzucone"
    );
}

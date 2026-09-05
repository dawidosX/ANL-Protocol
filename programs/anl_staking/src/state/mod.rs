//! Konta programu (spec v1.0, sekcja 3). Wersjonowane pod migracje (10F §12/§21).

use anchor_lang::prelude::*;

pub const ACCOUNT_VERSION: u8 = 1;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoolType {
    Flexible = 0,
    Genesis = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoolStatus {
    Active = 0,
    Paused = 1,
    Closed = 2,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PositionStatus {
    Active = 0,
    Closed = 1,
}

#[account]
pub struct GlobalConfig {
    pub version: u8,
    /// Multisig administracyjny.
    pub authority: Pubkey,
    /// ANL — Token-2022 (D-14).
    pub anl_mint: Pubkey,
    /// XNT — wrapped native X1, legacy SPL Token (D-14).
    pub xnt_mint: Pubkey,
    pub paused: bool,
    /// T0 okien APY; przy controlled rollout = planowany go-live (D-11, sekcja 7).
    pub genesis_start_ts: i64,
    /// Suma nagród ANL zarezerwowanych dla otwartych pozycji (WP §11:
    /// pokrycie w Reward Vault). Stake rezerwuje, claim/zerwanie zwalnia.
    pub anl_reward_reserved: u64,
    /// Operator (gorący klucz bota dziennego) — uprawniony WYŁĄCZNIE do
    /// fund_rewards / fund_xnt. Ustawiany przez authority (set_operator).
    /// Kompromitacja operatora nie zagraża środkom: może tylko wpłacać.
    pub operator: Pubkey,
    pub bump: u8,
    pub vault_authority_bump: u8,
    /// Skumulowana suma XNT wpłacona przez fund_xnt (amount_net, po opłatach).
    /// Rośnie monotonicznie, nie maleje przy wypłatach — źródło prawdy dla
    /// metryki "XNT rozdane" bez skanowania historii transakcji.
    pub total_xnt_funded: u64,
    /// CAPY (v3) - Token-2022, trzecia waluta (rozdzielony claim).
    pub capy_mint: Pubkey,
    pub capy_vault_bump: u8,
    /// Suma wyplaconych nagrod ANL (monotoniczna, tylko udany claim).
    pub total_anl_paid: u64,
    /// Suma naliczonego, niewyplaconego CAPY (pending userow).
    pub capy_reserved: u64,
    pub reserved: [u8; 16],
}

impl GlobalConfig {
    pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 32 + 1 + 1 + 8 + 32 + 1 + 8 + 8 + 16;
}

#[account]
pub struct PoolConfig {
    pub version: u8,
    pub pool_type: PoolType,
    pub status: PoolStatus,
    /// 6500 (Genesis) lub 3500 (Flexible).
    pub xnt_share_bps: u16,
    /// TVL puli — wartości NETTO po ewentualnych opłatach tokenowych.
    pub total_staked: u64,
    /// shares == total_staked (1:1, sekcja 6.1).
    pub total_shares: u64,
    /// Skumulowany indeks XNT × PRECISION.
    pub xnt_reward_index: u128,
    /// XNT przydzielone puli, gdy total_shares == 0 (D-5).
    pub xnt_undistributed: u64,
    pub position_count: u64,
    /// Epoka OSTATNIEGO fundingu tej puli (NO_EPOCH = nigdy). Audyt #2:
    /// indeks zmienia się wyłącznie w fund_xnt, więc snapshot końca każdej
    /// epoki E równa się indeksowi po ostatnim fundingu o epoce ≤ E.
    pub last_funded_epoch: u64,
    /// Epoka PIERWSZEGO fundingu (NO_EPOCH = nigdy) — dowód "zero fundingu
    /// ≤ end_epoch" bez konta checkpointu.
    pub first_funded_epoch: u64,
    pub bump: u8,
    /// Wariant A (koszyk dobowy): XNT skumulowany w BIEŻĄCEJ dobie, jeszcze
    /// nierozdzielony. close_day() dzieli go wg total_shares na KONIEC doby.
    pub current_day_basket: u64,
    /// Doba (epoka), do której należy current_day_basket.
    pub current_day: u64,
    pub reserved: [u8; 32],
}

impl PoolConfig {
    pub const LEN: usize = 8 + 1 + 1 + 1 + 2 + 8 + 8 + 16 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 32;

    /// Dzienny funding części tej puli (WP v1.0 §8). Przy `total_shares == 0`
    /// środki czekają w `xnt_undistributed` — zasada pustego koszyka.
    /// Wariant A — ZAMKNIĘCIE DOBY (leniwe). Rozdziela koszyk bieżącej doby
    /// wg total_shares NA KONIEC doby: index rośnie o basket/total_shares.
    /// Pusta pula → koszyk do bufora (nie ginie).
    pub fn close_day(&mut self) -> std::result::Result<(), anl_math::MathError> {
        if self.current_day_basket == 0 {
            return Ok(());
        }
        let basket = self.current_day_basket;
        if self.total_shares == 0 {
            self.xnt_undistributed = self
                .xnt_undistributed
                .checked_add(basket)
                .ok_or(anl_math::MathError::Overflow)?;
            self.current_day_basket = 0;
            return Ok(());
        }
        let part_total = self
            .xnt_undistributed
            .checked_add(basket)
            .ok_or(anl_math::MathError::Overflow)?;
        self.xnt_reward_index =
            anl_math::update_xnt_index(self.xnt_reward_index, part_total, self.total_shares)?;
        self.xnt_undistributed = 0;
        self.current_day_basket = 0;
        Ok(())
    }

    /// Wariant A — dodaje XNT do koszyka BIEŻĄCEJ doby. Nowa doba → najpierw
    /// zamyka poprzednią (rozdziela jej koszyk). NIE rusza index natychmiast.
    /// AUDYT R6 (R6-02): zwraca `Some(domknięta_doba)` WYŁĄCZNIE gdy ten
    /// wywołanie faktycznie rozdzieliło niepusty koszyk (jak
    /// `roll_day_if_needed`). Wołający zapisuje finalny indeks do checkpointu
    /// tylko wtedy — doba domknięta wcześniej ma już sfinalizowany checkpoint
    /// i nie może być nadpisana indeksem z późniejszymi redystrybucjami.
    pub fn add_to_basket(
        &mut self,
        part: u64,
        epoch: u64,
    ) -> std::result::Result<Option<u64>, anl_math::MathError> {
        let mut closed = None;
        if self.current_day != epoch {
            if self.current_day_basket > 0 {
                closed = Some(self.current_day);
            }
            self.close_day()?;
            self.current_day = epoch;
        }
        self.current_day_basket = self
            .current_day_basket
            .checked_add(part)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(closed)
    }

    /// Wariant A — wymusza zamknięcie doby, jeśli epoch jest nowszy niż doba
    /// koszyka. Wołane przez stake/settle PRZED zmianą total_shares.
    /// Wariant A (B) — zwraca Some(domknieta_doba), gdy faktycznie domknela
    /// koszyk. Wolajacy MUSI wtedy zapisac finalny index do checkpointu tej doby.
    pub fn roll_day_if_needed(
        &mut self,
        epoch: u64,
    ) -> std::result::Result<Option<u64>, anl_math::MathError> {
        if self.current_day != epoch && self.current_day_basket > 0 {
            let closed = self.current_day;
            self.close_day()?;
            self.current_day = epoch;
            return Ok(Some(closed));
        }
        Ok(None)
    }

    /// XNT należne pozycji przy bieżącym indeksie.
    pub fn pending_xnt(
        &self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        anl_math::pending_xnt(shares, self.xnt_reward_index, debt_index)
    }

    /// Settle po końcu okresu (WP §8): zamraża należność, zdejmuje shares —
    /// pozycja przestaje uczestniczyć w dziennej dystrybucji.
    pub fn settle_position(
        &mut self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let idx = self.xnt_reward_index;
        self.settle_position_at(shares, debt_index, idx)
    }

    /// Settlement względem HISTORYCZNEGO indeksu (checkpoint końca
    /// end_epoch) — fundamentalna poprawka audytu #1: funding po
    /// end_epoch nie może zwiększyć wypłaty pozycji.
    /// Zamyka pozycję na `cap_index` (ograniczony do jej end_epoch). Zwraca
    /// `pending` (XNT należny pozycji). Osierocony udział — to, co pula naliczyła
    /// tej pozycji ZA EPOKI PO jej end_epoch (bo index puli urósł wyżej niż cap
    /// po fundingach, których pozycja nie dożyła) — jest NATYCHMIAST rozdzielany
    /// shares żywym w tej chwili (AUDYT R5 M-01), a nie buforowany do
    /// następnego close_day (bufor rozdałby go także tym, którzy weszli później).
    pub fn settle_position_at(
        &mut self,
        shares: u64,
        debt_index: u128,
        cap_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        // AUDYT R7 (obrona w głąb dla R6-01): cap poniżej debt oznacza „brak
        // zafundowanej doby od wejścia" ⇒ pending 0, cały udział pozycji w
        // indeksie to orphan dla żywych. Miejsce kanoniczne to `cap_index_at`
        // (lifecycle.rs); tu zabezpieczamy model, by ŻADEN wołający nie mógł
        // zablokować rozliczenia błędem Overflow.
        let cap_index = cap_index.max(debt_index);
        let delta = cap_index - debt_index;
        let pending_u128 = delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        let pending: u64 = pending_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)?;
        let orphan_delta = self.xnt_reward_index.saturating_sub(cap_index);
        let orphan_u128 = orphan_delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        let orphan: u64 = orphan_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)?;
        self.total_shares = self
            .total_shares
            .checked_sub(shares)
            .ok_or(anl_math::MathError::Overflow)?;
        // AUDYT R5 (M-01): orphan trafia do ZYWYCH shares natychmiast (po
        // zdjeciu wychodzacego), nie do bezczasowego bufora, ktory nastepny
        // close_day rozdalby takze stakerom, ktorzy weszli PO tej dobie.
        self.redistribute_to_live(orphan)?;
        Ok(pending)
    }

    /// AUDYT R5 (M-01): rozdziela `amount` XNT (orphan po settle / przepadek
    /// po unstake_early) miedzy shares obecne W TEJ CHWILI - podnosi indeks.
    /// Odbiorcami sa wylacznie pozycje zywe w momencie wywolania; pozycje
    /// dojrzale z nizszym capem nie skorzystaja (ich nadwyzka wroci tu sama
    /// przy ich settle - kaskada zbiezna, konserwacja zachowana). Do
    /// `xnt_undistributed` trafia TYLKO gdy pula jest pusta (nie ma komu dac)
    /// - wtedy dostanie to pierwszy, kto wejdzie, zgodnie z zasada M-03.
    pub fn redistribute_to_live(
        &mut self,
        amount: u64,
    ) -> std::result::Result<(), anl_math::MathError> {
        if amount == 0 {
            return Ok(());
        }
        if self.total_shares == 0 {
            self.xnt_undistributed = self
                .xnt_undistributed
                .checked_add(amount)
                .ok_or(anl_math::MathError::Overflow)?;
            return Ok(());
        }
        self.xnt_reward_index =
            anl_math::update_xnt_index(self.xnt_reward_index, amount, self.total_shares)?;
        Ok(())
    }

    /// Genesis okna (WP okna 30-dniowe): policz XNT naliczony do `cap_index`
    /// BEZ zdejmowania shares — pozycja żyje dalej i nalicza w kolejnych oknach.
    /// Zwraca skumulowaną należność do progu (nie „do teraz"). Kwota do wypłaty
    /// w danym oknie = ta wartość MINUS `xnt_window_claimed` pozycji.
    /// `&self` — nic nie mutuje (w przeciwieństwie do settle_position_at).
    pub fn accrued_to_cap(
        &self,
        shares: u64,
        debt_index: u128,
        cap_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        // AUDYT R7 (obrona w głąb dla R6-01): jak w settle_position_at.
        let delta = cap_index.max(debt_index) - debt_index;
        let acc_u128 = delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        acc_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)
    }

    /// Wcześniejsze zerwanie (WP §7): naliczone XNT przepadają na rzecz
    /// pozostałych — AUDYT R5: rozdzielone natychmiast żywym shares
    /// (`settle_position` już zdjął shares wychodzącego), bufor tylko przy
    /// pustej puli.
    pub fn forfeit_position(
        &mut self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let pending = self.settle_position(shares, debt_index)?;
        // AUDYT R5: przepadek tez natychmiast do zywych (bufor tylko przy pustej puli)
        self.redistribute_to_live(pending)?;
        Ok(pending)
    }
}

#[account]
pub struct UserPosition {
    pub version: u8,
    pub owner: Pubkey,
    pub pool_type: PoolType,
    pub status: PositionStatus,
    pub position_index: u64,
    /// Principal NETTO (actual received — sekcja 9).
    pub amount: u64,
    pub shares: u64,
    /// Immutable APY (TC-049) — przypisane w chwili otwarcia, na zawsze.
    pub apy_bps: u16,
    /// Okres deklarowany przez uczestnika — OBA programy (WP v1.0 §7).
    pub declared_days: u32,
    pub start_ts: i64,
    /// Koniec zadeklarowanego okresu; po tej chwili naliczanie stoi.
    pub end_ts: i64,
    /// Nagroda ANL pozycji, wyliczona i zarezerwowana przy otwarciu
    /// (Immutable APY ⇒ kwota znana z góry; WP §7).
    pub anl_reward: u64,
    /// XNT zamrożone przy settle po końcu okresu (WP §8).
    pub xnt_accrued: u64,
    /// Pozycja rozliczona z koszyka XNT (shares zdjęte po end_ts).
    pub settled: bool,
    /// Snapshot xnt_reward_index z chwili wejścia.
    pub xnt_debt_index: u128,
    pub bump: u8,
    /// Epoka XNT zawierająca ostatnią naliczaną sekundę pozycji
    /// AUDYT R4: `epoch_of(end_ts) - 1` — ostatnia W PEŁNI przesiedziana doba
    /// (koniec w środku doby K ⇒ K-1; dokładnie na granicy ⇒ K). Settlement używa
    /// checkpointu ≤ end_epoch. Pozycje sprzed deployu R4 mają starą formułę.
    pub end_epoch: u64,
    /// Genesis: skumulowana suma XNT już wypłacona w oknach 30-dniowych.
    /// Zapewnia kumulację i chroni przed podwójną wypłatą (WP okna Genesis).
    pub xnt_window_claimed: u64,
    /// Genesis: timestamp ostatniej wypłaty okienkowej (0 = nigdy).
    pub last_window_ts: i64,
    pub reserved: [u8; 8],
}

impl UserPosition {
    pub const LEN: usize =
        8 + 1 + 32 + 1 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 8 + 1 + 16 + 1 + 8 + 8 + 8 + 8;
}

#[account]
pub struct UserProfile {
    pub owner: Pubkey,
    pub next_position_index: u64,
    pub bump: u8,
    /// CAPY (v3): naliczone, nieodebrane CAPY. Przezywa close pozycji.
    pub pending_capy: u64,
    pub reserved: [u8; 7],
}

impl UserProfile {
    pub const LEN: usize = 8 + 32 + 8 + 1 + 8 + 7;
}

/// Snapshot indeksu puli po epoce, w której wystąpił funding.
/// PDA: [XNT_CKPT_SEED, pool_type, epoch.to_le_bytes()].
/// `next_funded_epoch` (NO_EPOCH = brak) tworzy łańcuch dowodowy:
/// checkpoint K jest ostatnim fundingiem ≤ E ⟺ K.epoch ≤ E oraz
/// (K.next == NO_EPOCH ∨ K.next > E).
#[account]
pub struct XntCheckpoint {
    pub version: u8,
    pub pool_type: PoolType,
    pub epoch: u64,
    /// xnt_reward_index puli po WSZYSTKICH fundingach tej epoki.
    pub index: u128,
    pub next_funded_epoch: u64,
    pub bump: u8,
    pub reserved: [u8; 13],
}

impl XntCheckpoint {
    pub const LEN: usize = 8 + 1 + 1 + 8 + 16 + 8 + 1 + 13;
}

/// Numer epoki XNT dla chwili `ts` względem genesis (epoka = 1 dzień,
/// granice zsynchronizowane z oknami Genesis o 02:00 UTC).
pub fn epoch_of(ts: i64, genesis_start_ts: i64) -> Option<u64> {
    if ts < genesis_start_ts {
        return None;
    }
    Some(((ts - genesis_start_ts) as u64) / (anl_math::SECONDS_PER_DAY as u64))
}

/// AUDYT R4 (H-01/M-01): wspólny krok "przekręć dobę wg ZEGARA + sfinalizuj
/// checkpoint domkniętej doby". Wołany przez stake / claim / settle_expired
/// PRZED liczeniem capu i PRZED zmianą total_shares — dzięki temu:
///   1. `current_day` nigdy nie jest stale względem zegara na ścieżkach
///      wyjścia (cap nie może być zaniżony o zafundowane, niedomknięte doby),
///   2. kolejność transakcji (`close_day` vs `settle`/`claim`) nie zmienia
///      wypłat — obie ścieżki konwergują do tego samego stanu indeksu.
///
/// Gdy roll faktycznie domyka dobę (koszyk > 0), checkpoint tej doby MUSI
/// zostać podany i zostaje nadpisany finalnym indeksem (checkpoint istnieje,
/// bo koszyk > 0 ⇒ fund_xnt tej doby go utworzył). Walidacja: owner, PDA,
/// wersja/epoka/pool_type — fail-closed jak w fund::write_final_index.
pub fn roll_day_and_write_checkpoint<'info>(
    pool: &mut PoolConfig,
    cur_epoch: u64,
    prev_day_ckpt: Option<&UncheckedAccount<'info>>,
    program_id: &Pubkey,
) -> Result<()> {
    use crate::constants::XNT_CKPT_SEED;
    use crate::errors::AnlError;

    let closed = pool.roll_day_if_needed(cur_epoch).map_err(AnlError::from)?;
    if let Some(closed_epoch) = closed {
        let final_index = pool.xnt_reward_index;
        let pool_type = pool.pool_type;
        let ai = prev_day_ckpt.ok_or(AnlError::CheckpointRequired)?;
        let info = ai.to_account_info();
        require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
        let (pda, _) = Pubkey::find_program_address(
            &[
                XNT_CKPT_SEED,
                &[pool_type as u8],
                &closed_epoch.to_le_bytes(),
            ],
            program_id,
        );
        require_keys_eq!(info.key(), pda, AnlError::CheckpointMismatch);
        let mut ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
        require!(
            ck.version == ACCOUNT_VERSION && ck.epoch == closed_epoch && ck.pool_type == pool_type,
            AnlError::CheckpointMismatch
        );
        ck.index = final_index;
        ck.try_serialize(&mut &mut info.data.borrow_mut()[..])?;
    }
    Ok(())
}

#[cfg(test)]
mod wariant_a_tests {
    use super::*;
    use crate::constants::NO_EPOCH;

    fn empty_pool(pool_type: PoolType) -> PoolConfig {
        PoolConfig {
            version: ACCOUNT_VERSION,
            pool_type,
            status: PoolStatus::Active,
            xnt_share_bps: 6500,
            total_staked: 0,
            total_shares: 0,
            xnt_reward_index: 0,
            xnt_undistributed: 0,
            position_count: 0,
            last_funded_epoch: NO_EPOCH,
            first_funded_epoch: NO_EPOCH,
            bump: 0,
            current_day_basket: 0,
            current_day: 0,
            reserved: [0; 32],
        }
    }

    fn pending(pool: &PoolConfig, shares: u64, debt: u128) -> u64 {
        ((pool.xnt_reward_index - debt) * (shares as u128) / anl_math::PRECISION) as u64
    }

    #[test]
    fn test_podzial_wg_finalnych_shares() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt1: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        let debt2: u128 = pool.xnt_reward_index;
        pool.total_shares += 1_000_000;
        pool.roll_day_if_needed(1).unwrap();
        let xnt1 = pending(&pool, 1_000_000, debt1);
        let xnt2 = pending(&pool, 1_000_000, debt2);
        assert_eq!(xnt1, 5_000_000_000, "#1 powinien dostac 5 XNT");
        assert_eq!(xnt2, 5_000_000_000, "#2 powinien dostac 5 XNT");
    }

    #[test]
    fn test_kumulacja_przez_dwie_doby() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(6_000_000_000, 0).unwrap();
        pool.add_to_basket(4_000_000_000, 1).unwrap();
        pool.roll_day_if_needed(2).unwrap();
        let xnt = pending(&pool, 1_000_000, debt);
        assert_eq!(xnt, 10_000_000_000, "pozycja przez 2 doby dostaje 10 XNT");
    }

    #[test]
    fn test_pusta_pula_koszyk_do_bufora() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        pool.roll_day_if_needed(1).unwrap();
        assert_eq!(
            pool.xnt_undistributed, 10_000_000_000,
            "pusta pula -> bufor"
        );
        assert_eq!(
            pool.xnt_reward_index, 0,
            "index nie rosnie przy pustej puli"
        );
        let debt: u128 = pool.xnt_reward_index;
        pool.total_shares = 1_000_000;
        pool.add_to_basket(5_000_000_000, 1).unwrap();
        pool.roll_day_if_needed(2).unwrap();
        let xnt = pending(&pool, 1_000_000, debt);
        assert_eq!(
            xnt, 15_000_000_000,
            "staker dostaje bufor 10 + koszyk 5 = 15 XNT"
        );
    }

    #[test]
    fn test_pozny_staker_nie_lapie_zamknietej_doby() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt1: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        pool.roll_day_if_needed(1).unwrap();
        let debt2: u128 = pool.xnt_reward_index;
        pool.total_shares += 1_000_000;
        let xnt1 = pending(&pool, 1_000_000, debt1);
        let xnt2 = pending(&pool, 1_000_000, debt2);
        assert_eq!(
            xnt1, 10_000_000_000,
            "#1 dostaje cale 10 (byl sam w dobie 0)"
        );
        assert_eq!(xnt2, 0, "#2 nie lapie doby 0 (wszedl w dobie 1)");
    }

    // ===== AUDYT R4: determinizm ostatniej doby (H-01 raportow B i C) =====
    //
    // Model: pozycja A (100 shares) konczy sie W SRODKU doby E => po fixie
    // end_epoch = E-1 (tylko pelne doby). Pozycja B (100 shares) zyje dalej.
    // Koszyk doby E = 100 XNT. Cap A = finalny indeks doby E-1.
    // Wymog: A i B dostaja TO SAMO niezaleznie od kolejnosci
    // (roll/close przed settle vs settle przed close).

    const SHARES: u64 = 100;
    const BASKET_E: u64 = 100_000_000_000; // 100 XNT

    /// Pula na poczatku doby E: A+B w srodku, doby <E rozliczone (index=idx0).
    fn pool_at_day_e(idx0: u128) -> PoolConfig {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 2 * SHARES;
        pool.xnt_reward_index = idx0;
        pool.current_day = 5; // E = 5
        pool.current_day_basket = 0;
        pool
    }

    #[test]
    fn test_r4_kolejnosc_settle_vs_close_identyczna_wyplata() {
        let idx0: u128 = 7 * anl_math::PRECISION; // dowolny finalny indeks E-1
        let debt: u128 = 0;

        // --- Kolejnosc 1: dzien E domkniety PRZED settle A ---
        let mut p1 = pool_at_day_e(idx0);
        p1.add_to_basket(BASKET_E, 5).unwrap();
        p1.roll_day_if_needed(6).unwrap(); // koniec doby E: koszyk w indeks
        let cap_a = idx0; // cap A = finalny indeks doby E-1 (end_epoch = 4)
        let a1 = p1.settle_position_at(SHARES, debt, cap_a).unwrap();
        let b1 = pending(&p1, SHARES, debt);
        // orphan A (wzrost indeksu za dobe E ponad cap) wrocil do undistributed
        let und1 = p1.xnt_undistributed;

        // --- Kolejnosc 2: settle A PRZED domknieciem doby E ---
        let mut p2 = pool_at_day_e(idx0);
        p2.add_to_basket(BASKET_E, 5).unwrap();
        let a2 = p2.settle_position_at(SHARES, debt, cap_a).unwrap();
        p2.roll_day_if_needed(6).unwrap(); // teraz koszyk E idzie na same B
        let b2 = pending(&p2, SHARES, debt);
        let und2 = p2.xnt_undistributed;

        assert_eq!(a1, a2, "A: wyplata zalezna od kolejnosci (H-01!)");
        // B: kolejnosc 1 dostaje polowe koszyka E od razu + polowa przez
        // orphan->undistributed przy nastepnym fundingu; kolejnosc 2 dostaje
        // caly koszyk E od razu. Suma dla B musi byc rowna:
        let b1_total = b1 + und1;
        let b2_total = b2 + und2;
        assert_eq!(
            b1_total, b2_total,
            "B: laczna pula (index + undistributed) zalezna od kolejnosci"
        );
        // Konserwacja: A + pula B + undistributed == idx0*shares + koszyk E
        let baza = 2 * (idx0 * SHARES as u128 / anl_math::PRECISION) as u64;
        assert_eq!(a1 + b1 + und1, baza + BASKET_E, "kolejnosc 1: wyciek XNT");
        assert_eq!(a2 + b2 + und2, baza + BASKET_E, "kolejnosc 2: wyciek XNT");
    }

    #[test]
    fn test_r4_stale_current_day_roll_oddaje_zafundowana_dobe() {
        // Funding w dobie 5, zegar w dobie 10, nikt nie zamknal doby.
        // Pozycja z end_epoch >= 5 PO rollu (jak w claim/settle po fixie)
        // musi dostac koszyk doby 5.
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = SHARES;
        let debt: u128 = pool.xnt_reward_index;
        pool.current_day = 5;
        pool.add_to_basket(BASKET_E, 5).unwrap();
        // PRZED fixem: cap liczony przy current_day=5, basket>0
        // => last_closed=4 => pozycja tracila cala dobe 5 na zawsze.
        // PO fixie: settle/claim najpierw roluja wg zegara:
        let closed = pool.roll_day_if_needed(10).unwrap();
        assert_eq!(closed, Some(5), "roll musi domknac stale dobe 5");
        assert_eq!(pool.current_day, 10);
        assert_eq!(pool.current_day_basket, 0);
        let xnt = pending(&pool, SHARES, debt);
        assert_eq!(xnt, BASKET_E, "pozycja dostaje pelna zafundowana dobe 5");
    }

    // ===== AUDYT R5 (M-01 raport C): orphan do ZYWYCH, nie do bufora =====
    const M: u64 = 1_000_000;
    const DAY_XNT: u64 = 100_000_000_000; // 100 XNT

    #[test]
    fn test_r5_orphan_natychmiast_do_zywych_nie_do_pozniejszych() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 2 * M; // Adam + Beata
        let debt_ab: u128 = pool.xnt_reward_index;
        pool.current_day = 5;
        pool.add_to_basket(DAY_XNT, 5).unwrap();
        pool.roll_day_if_needed(6).unwrap(); // close_day(5): po 50 dla A i B
        let cap_adam = debt_ab; // Adam: cap = indeks sprzed doby 5
        let adam_paid = pool.settle_position_at(M, debt_ab, cap_adam).unwrap();
        assert_eq!(adam_paid, 0, "Adam nie ma prawa do doby 5");
        assert_eq!(pool.xnt_undistributed, 0, "orphan NIE laduje w buforze");
        assert_eq!(pool.total_shares, M, "zostala Beata");
        assert_eq!(
            pending(&pool, M, debt_ab),
            DAY_XNT,
            "Beata ma CALE 100 z doby 5"
        );
        let debt_c: u128 = pool.xnt_reward_index; // Celina wchodzi w dobie 6
        pool.total_shares += M;
        pool.add_to_basket(DAY_XNT, 6).unwrap();
        pool.roll_day_if_needed(7).unwrap(); // close_day(6): 100 na B i C po 50
        assert_eq!(
            pending(&pool, M, debt_ab),
            DAY_XNT + DAY_XNT / 2,
            "Beata: 100 + 50"
        );
        assert_eq!(
            pending(&pool, M, debt_c),
            DAY_XNT / 2,
            "Celina: TYLKO 50 z doby 6"
        );
        assert_eq!(
            adam_paid + pending(&pool, M, debt_ab) + pending(&pool, M, debt_c),
            2 * DAY_XNT
        );
    }

    #[test]
    fn test_r5_orphan_pusta_pula_idzie_do_bufora() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = M;
        let debt: u128 = pool.xnt_reward_index;
        pool.current_day = 5;
        pool.add_to_basket(DAY_XNT, 5).unwrap();
        pool.roll_day_if_needed(6).unwrap();
        let paid = pool.settle_position_at(M, debt, debt).unwrap();
        assert_eq!(paid, 0);
        assert_eq!(pool.total_shares, 0);
        assert_eq!(
            pool.xnt_undistributed, DAY_XNT,
            "pusta pula: orphan czeka na pierwszego"
        );
    }

    #[test]
    fn test_r5_przepadek_early_exit_do_zywych() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 2 * M;
        let debt: u128 = pool.xnt_reward_index;
        pool.current_day = 5;
        pool.add_to_basket(DAY_XNT, 5).unwrap();
        pool.roll_day_if_needed(6).unwrap();
        let forfeited = pool.forfeit_position(M, debt).unwrap();
        assert_eq!(forfeited, DAY_XNT / 2, "A traci swoje 50");
        assert_eq!(pool.xnt_undistributed, 0);
        assert_eq!(pending(&pool, M, debt), DAY_XNT, "B ma cale 100 od razu");
    }

    // ===== AUDYT R7: obrona w glab R6-01 na poziomie modelu =====
    #[test]
    fn test_r7_cap_ponizej_debt_daje_zero_bez_overflow() {
        let p = anl_math::PRECISION;
        // Po redystrybucji indeks = 200, wchodzi pozycja (debt 200); jej cap
        // wskazuje na checkpoint sfinalizowany wczesniej (100). Przed R7 model
        // zwracal Overflow (blokada claim); teraz: pending 0, orphan = udzial
        // od wejscia (0 tutaj), indeks i zywi nietknieci.
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 2 * M;
        pool.xnt_reward_index = 200 * p;
        assert_eq!(pool.accrued_to_cap(M, 200 * p, 100 * p).unwrap(), 0);
        let paid = pool.settle_position_at(M, 200 * p, 100 * p).unwrap();
        assert_eq!(paid, 0, "cap < debt => nic nie nalezne");
        assert_eq!(pool.total_shares, M, "shares wychodzacego zdjete");
        assert_eq!(
            pool.xnt_reward_index,
            200 * p,
            "orphan 0 => indeks bez zmian"
        );
        assert_eq!(pool.xnt_undistributed, 0);
        // Przypadek normalny (cap > debt) bez zmian semantyki:
        let mut p2 = empty_pool(PoolType::Genesis);
        p2.total_shares = M;
        p2.xnt_reward_index = 300 * p;
        assert_eq!(p2.accrued_to_cap(M, 100 * p, 250 * p).unwrap(), 150 * M);
        let paid2 = p2.settle_position_at(M, 100 * p, 250 * p).unwrap();
        assert_eq!(paid2, 150 * M, "(250-100) x shares");
        assert_eq!(p2.total_shares, 0);
        assert_eq!(
            p2.xnt_undistributed,
            50 * M,
            "orphan (300-250) x shares do bufora (pusta pula)"
        );
    }

    // ===== AUDYT R6: property-test KONSERWACJI XNT (Inwariant 10) =====
    //
    // Losowe sekwencje {stake, fund, roll, settle_expired(cap), forfeit}
    // na modelu puli z deterministycznym LCG. Po KAZDYM kroku:
    //   wyplacone + pending(zywi) + undistributed + basket + dust == zafundowane
    // gdzie dust >= 0 (floor) i dust jest OGRANICZONY liczba operacji
    // dzielenia * total_shares/PRECISION (kazde dzielenie gubi < 1 jednostke
    // na share). Zaden krok nie moze zwiekszyc sumy roszczen ponad wplaty.

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    #[test]
    fn test_r6_property_konserwacja_xnt_losowe_sekwencje() {
        for seed in 1u64..=40 {
            let mut rng = Lcg(seed);
            let mut pool = empty_pool(PoolType::Genesis);
            // pozycje: (shares, debt, cap_epoch) ; cap = indeks na koniec cap_epoch
            let mut live: Vec<(u64, u128, u64)> = Vec::new();
            let mut ckpt_final: Vec<u128> = Vec::new(); // indeks po domknieciu doby e
            let mut funded: u64 = 0;
            let mut paid: u64 = 0;
            let mut epoch: u64 = 0;
            let mut divisions: u64 = 0;
            pool.current_day = 0;
            for _step in 0..120 {
                match rng.below(5) {
                    0 => {
                        // stake: shares w [1e9, 5e12]
                        let sh = 1_000_000_000 + rng.below(5_000_000_000_000);
                        pool.total_shares += sh;
                        let cap_e = epoch + 1 + rng.below(6);
                        live.push((sh, pool.xnt_reward_index, cap_e));
                    }
                    1 => {
                        // funding biezacej doby (moze byc kilka)
                        let amt = 1 + rng.below(50_000_000_000);
                        pool.add_to_basket(amt, epoch).unwrap();
                        funded += amt;
                    }
                    2 => {
                        // koniec doby: roll (close_day) + zapis "checkpointu"
                        if pool.current_day_basket > 0 && pool.total_shares > 0 {
                            divisions += 1;
                        }
                        epoch += 1;
                        let _ = pool.roll_day_if_needed(epoch).unwrap();
                        while ckpt_final.len() < epoch as usize {
                            ckpt_final.push(pool.xnt_reward_index);
                        }
                    }
                    3 => {
                        // settle_expired pozycji, ktorej cap_epoch juz minal
                        if let Some(i) = live.iter().position(|p| p.2 < epoch) {
                            let (sh, debt, cap_e) = live.remove(i);
                            let cap = ckpt_final[cap_e as usize].max(debt);
                            let out = pool.settle_position_at(sh, debt, cap).unwrap();
                            paid += out;
                            if pool.total_shares > 0 {
                                divisions += 1;
                            }
                        }
                    }
                    _ => {
                        // forfeit (early exit) losowej zywej pozycji
                        if !live.is_empty() {
                            let i = rng.below(live.len() as u64) as usize;
                            let (sh, debt, _) = live.remove(i);
                            let _ = pool.forfeit_position(sh, debt).unwrap();
                            if pool.total_shares > 0 {
                                divisions += 1;
                            }
                        }
                    }
                }
                // ---- INWARIANT 10 po kazdym kroku ----
                let pending_live: u64 = live
                    .iter()
                    .map(|(sh, debt, _)| pending(&pool, *sh, *debt))
                    .sum();
                let claims = paid + pending_live + pool.xnt_undistributed + pool.current_day_basket;
                assert!(
                    claims <= funded,
                    "seed {seed}: roszczenia {claims} > wplaty {funded} (WYCIEK)"
                );
                // dust: kazde dzielenie gubi < total_shares/PRECISION jednostek;
                // shares <= 40*5e12 => < 1 jednostka na dzielenie przy 1e12
                let dust = funded - claims;
                let bound = divisions
                    .saturating_mul(1 + pool.total_shares / anl_math::PRECISION as u64)
                    + live.len() as u64; // + floor per pending
                assert!(
                    dust <= bound + 200,
                    "seed {seed}: dust {dust} > ograniczenie {bound} (zaginione XNT)"
                );
            }
        }
    }
}

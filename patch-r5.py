import sys, re
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
if 'redistribute_to_live' in s:
    print('JUZ ZALATANE'); sys.exit(0)
ch = []
old1 = """        let orphan: u64 = orphan_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)?;
        self.xnt_undistributed = self.xnt_undistributed.saturating_add(orphan);
        self.total_shares = self
            .total_shares
            .checked_sub(shares)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(pending)
    }"""
new1 = """        let orphan: u64 = orphan_u128
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
    }"""
assert old1 in s, 'A settle_position_at'
s = s.replace(old1, new1, 1); ch.append('A orphan->zywi')
old2 = """        let pending = self.settle_position(shares, debt_index)?;
        self.xnt_undistributed = self
            .xnt_undistributed
            .checked_add(pending)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(pending)
    }"""
new2 = """        let pending = self.settle_position(shares, debt_index)?;
        // AUDYT R5: przepadek tez natychmiast do zywych (bufor tylko przy pustej puli)
        self.redistribute_to_live(pending)?;
        Ok(pending)
    }"""
assert old2 in s, 'B forfeit'
s = s.replace(old2, new2, 1); ch.append('B przepadek->zywi')
old3 = """    /// po fundingach, których pozycja nie dożyła) — wraca do `xnt_undistributed`,
    /// żeby przy następnym fundingu rozdał się żywym. Bez tego osiadałby w skarbcu
    /// jako niewypłacalny nadmiar."""
new3 = """    /// po fundingach, których pozycja nie dożyła) — jest NATYCHMIAST rozdzielany
    /// shares żywym w tej chwili (AUDYT R5 M-01), a nie buforowany do
    /// następnego close_day (bufor rozdałby go także tym, którzy weszli później)."""
if old3 in s:
    s = s.replace(old3, new3, 1); ch.append('C doc settle')
m = re.search(r"( *)/// \(epoch_of\(end_ts - 1\)\)\. Settlement używa checkpointu ≤ end_epoch\.\n", s)
if m:
    ind = m.group(1)
    s = s.replace(m.group(0), ind+"/// AUDYT R4: `epoch_of(end_ts) - 1` — ostatnia W PEŁNI przesiedziana doba\n"+ind+"/// (koniec w środku doby K ⇒ K-1; dokładnie na granicy ⇒ K). Settlement używa\n"+ind+"/// checkpointu ≤ end_epoch. Pozycje sprzed deployu R4 mają starą formułę.\n", 1)
    ch.append('D doc end_epoch')
old4 = """        let xnt = pending(&pool, SHARES, debt);
        assert_eq!(xnt, BASKET_E, "pozycja dostaje pelna zafundowana dobe 5");
    }
}"""
new4 = """        let xnt = pending(&pool, SHARES, debt);
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
        assert_eq!(pending(&pool, M, debt_ab), DAY_XNT, "Beata ma CALE 100 z doby 5");
        let debt_c: u128 = pool.xnt_reward_index; // Celina wchodzi w dobie 6
        pool.total_shares += M;
        pool.add_to_basket(DAY_XNT, 6).unwrap();
        pool.roll_day_if_needed(7).unwrap(); // close_day(6): 100 na B i C po 50
        assert_eq!(pending(&pool, M, debt_ab), DAY_XNT + DAY_XNT / 2, "Beata: 100 + 50");
        assert_eq!(pending(&pool, M, debt_c), DAY_XNT / 2, "Celina: TYLKO 50 z doby 6");
        assert_eq!(adam_paid + pending(&pool, M, debt_ab) + pending(&pool, M, debt_c), 2 * DAY_XNT);
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
        assert_eq!(pool.xnt_undistributed, DAY_XNT, "pusta pula: orphan czeka na pierwszego");
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
}"""
assert old4 in s, 'E testy'
s = s.replace(old4, new4, 1); ch.append('E testy R5')
open(p, 'w', encoding='utf-8').write(s)
print('OK:', ', '.join(ch))

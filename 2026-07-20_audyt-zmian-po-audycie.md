# Audyt zmian po-audytowych — rozbicie `initialize` (fix stack overflow SBF)

**Data:** 2026-07-20
**Zakres:** zmiany w kodzie kontraktu wprowadzone 20.07.2026, po rundzie audytowej #4
**Powód zmian:** instrukcja `Initialize::try_accounts` przepełniała stos SBF (ramka 6720–7232 B przy limicie 4096) — program kompilował się, ale `initialize` był niewykonywalny na łańcuchu (`Access violation in stack frame`).
**Cel audytu:** ustalić, czy naprawa nie wprowadziła nowych wektorów ataku, szczególnie w nowym stanie pośrednim.

---

## Podsumowanie wykonawcze

Zmiany są **niefunkcjonalne dla logiki protokołu** — rozbijają jedną instrukcję setupu na cztery mniejsze, nie zmieniając reguł biznesowych, inwariantów skarbców ani mechaniki nagród. Wprowadzają jeden nowy element do przeanalizowania: **stan pośredni** między `initialize` a `init_*_vault`, w którym GlobalConfig istnieje, a skarbce jeszcze nie. Analiza wykazała, że stan ten jest **bezpieczny** dzięki trzem warstwom ochrony (patrz A-01). Nie znaleziono nowych luk krytycznych ani wysokich. Zmiana wymaga jednak formalnej re-weryfikacji przez zewnętrznych audytorów przed mainnetem (patrz rekomendacje).

**Werdykt: zmiany bezpieczne dla testnetu. Przed mainnetem — wymagana re-weryfikacja rundy audytowej.**

---

## Zakres zmian (7 plików)

| Plik | Zmiana | Klasa |
|---|---|---|
| `lib.rs` | Program ID testnet + rejestracja 3 nowych instrukcji | strukturalna |
| `initialize.rs` | Rozbicie `Initialize` na `Initialize` + `InitPrincipalVault` + `InitRewardVault` + `InitXntVault`; boxowanie | **strukturalna (audytowana instrukcja)** |
| `stake.rs`, `lifecycle.rs`, `fund.rs` | Boxowanie kont (`Box<Account>`, `Box<InterfaceAccount>`) | pamięciowa |
| `Cargo.toml` | anchor-lang/spl 0.29.0 → 0.30.1 | zależności |
| `Cargo.lock` | konsekwencja powyższego | zależności |

---

## Ustalenia

### A-01 · Stan pośredni initialize↔init_vaults — BEZPIECZNY (było: potencjalnie wysokie)

**Ryzyko:** po `initialize` istnieje GlobalConfig, ale skarbce (principal/reward/xnt) jeszcze nie. Pytanie: czy atakujący może wykorzystać to okno?

**Analiza — trzy warstwy ochrony:**

1. **Front-run tworzenia skarbców niemożliwy.** Każda z instrukcji `InitPrincipalVault`, `InitRewardVault`, `InitXntVault` waliduje GlobalConfig przez `has_one = authority @ InvalidAuthority`. Tylko `authority` zapisane w GlobalConfig (ustawione w `initialize`) może utworzyć skarbce. Obcy podpisujący → `InvalidAuthority`. Zweryfikowano: guard obecny we wszystkich trzech strukturach.

2. **Skarbce to PDA o stałych seeds** (`PRINCIPAL_VAULT_SEED`, `REWARD_VAULT_SEED`, `XNT_VAULT_SEED`), z `token::authority = vault_authority` (PDA). Atakujący nie może podłożyć własnego konta jako skarbca — adres jest zdeterminowany przez program, a authority to PDA, nie klucz atakującego.

3. **Stake przed utworzeniem skarbców zawodzi bezpiecznie.** W instrukcji `Stake` konta `principal_vault`/`reward_vault` są typu `Box<InterfaceAccount<TokenAccount>>` **bez `init`** — wymagają istniejącego, zainicjalizowanego konta. Próba stake w oknie pośrednim → `AccountNotInitialized`. Brak ścieżki do zapisu principalu do nieistniejącego skarbca.

**Wniosek:** okno pośrednie nie daje atakującemu żadnej akcji. Najgorszy scenariusz to authority nie kończące setupu (własny błąd operacyjny, nie luka) — protokół pozostaje wtedy w stanie „bez skarbców", w którym nikt nie może stakować. Naprawialne dokończeniem `init_*_vault`. **Nie stanowi luki bezpieczeństwa.**

### A-02 · Integralność walidacji przy boxowaniu — POTWIERDZONA

`Box<T>` zmienia wyłącznie miejsce alokacji (stos → sterta) przez `Deref`/`DerefMut`; nie dotyka constraints Anchora. Zweryfikowano liczbę reguł walidacji (`constraint`/`has_one`/`address`) per plik — wszystkie zachowane: initialize 10, stake 4, lifecycle 16, fund 12. Boxowanie nie usunęło żadnej reguły. Handlery odwołują się do pól przez auto-deref — logika bez zmian.

### A-03 · Migracja anchor 0.29→0.30.1 — BEZ REGRESJI

Kod używał już API 0.30 (`ctx.bumps.<nazwa>` jako pola nazwane, nie `.get()`), więc migracja nie wymagała zmian w handlerach. `anchor-spl 0.30` pociąga `spl-token-2022 3.x` — kompiluje się czysto. Dyskryminatory instrukcji liczone z `sha256("global:<name>")` — niezmienne między wersjami Anchora dla tych samych nazw.

### A-04 · Nowy Program ID testnet — KOSMETYKA

`declare_id!` testnet zmieniony na nowy adres (reset stanu pod tokenomię 1B). Bez wpływu na logikę. **Uwaga dokumentacyjna:** `Anchor.toml` (linia 13) i `docs/SECURITY-AUDITS(.pl).md` (6 miejsc) nadal wskazują stary Program ID — wymaga aktualizacji przed pushem, inaczej dokumentacja kłamie.

---

## Rekomendacje przed pushem na publiczne repo

1. **[MUST] Uczciwy commit message** — jawnie opisać rozbicie `initialize` jako zmianę po-audytową (powód: stack overflow), nie „drobną poprawkę".
2. **[MUST] Wpis w SECURITY-AUDITS** — odnotować, że `initialize` zmienił strukturę po rundzie #4 i **wymaga re-weryfikacji przed mainnetem** (nie ukrywać po-audytowego charakteru).
3. **[MUST] Zaktualizować Program ID** w `Anchor.toml` + SECURITY-AUDITS (stary→nowy albo oznaczyć jako testnet-efemeryczny).
4. **[SHOULD] Zaktualizować testy integracyjne** — `tests/integration.rs` woła stary jednoetapowy `initialize`; po rozbiciu wymaga wywołania 4 instrukcji. Inaczej CI zapłonie.
5. **[SHOULD] Rozważyć branch `testnet`** zamiast `main` — by `main` pozostał tożsamy z kodem audytowanym, a zmiany robocze żyły osobno do czasu re-audytu.
6. **[CONSIDER] Testnetowe adresy tokenów poza repo** — efemeryczne (dwa resety w jeden dzień), lepiej na stronie testnetowej niż w wersjonowanym kodzie.

---

## Czego ten audyt NIE obejmuje

- Pełnego re-audytu logiki protokołu (nagrody, APY, cykl życia) — te części kodu nietknięte od rundy #4, ale formalna re-weryfikacja całości należy do zewnętrznych audytorów.
- Testów on-chain drugiej warstwy (fund_xnt, claim, unstake_early) — sprawdzono deploy i stake; pełny cykl wymaga dojrzenia pozycji.
- Analizy formalnej/fuzzingu — poza zakresem tej szybkiej weryfikacji zmian.

**Ten dokument to wewnętrzna analiza zmian, nie zastępuje formalnego audytu zewnętrznego przed mainnetem.**

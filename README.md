# XNMiner m=10000+ — DAWIDOS.X1 · ANL Protocol (Tony.x1 build)

> **Wersja:** v5.0.0-m10000
> **Data:** 2026-08-28
> **Cel:** Kopanie pod aktualny diff sieci XenBlocks (min m=10000), auto-follow.

Sieć XenBlocks podniosła minimalne difficulty na **m=10000** (oscyluje 10000-16000 wg liczby minerów, nigdy niżej). Stary tryb harvest m=100 przestał działać (bloki m=100 nie wchodzą on-chain). Ten build produkuje pod **aktualny diff sieci** (auto-follow), żeby bloki były akceptowane.

---

## Szybki start

### 1. Zależności
```bash
sudo apt-get update
sudo apt-get install -y build-essential cmake libcurl4-openssl-dev nlohmann-json3-dev
# lub: ./install-deps.sh
```

### 2. Build
```bash
./build.sh
# wynik: build/xnminer-cuda (albo podobny binary w build/)
```

### 3. Config — wybierz właściwy

**Na vast.ai / cloud (port 80 często blokowany):**
```bash
cp miner.ini.vast miner.ini
```

**Na W1-W4 / HiveOS produkcja (port 80 działa):**
```bash
cp miner.ini.prod miner.ini
# dostosuj worker= i device_id= per karta (0-7)
```

### 4. Start
```bash
./start-miner.sh
# lub: nohup ./build/xnminer-cuda miner.ini > /tmp/miner.log 2>&1 &
```

---

## KLUCZOWE — różnica vast vs produkcja

| Parametr | vast (miner.ini.vast) | produkcja (miner.ini.prod) |
|----------|----------------------|---------------------------|
| `memory_cost` | 10000 | 10000 |
| `force_mine_memory_cost` | 0 (auto-follow) | 0 (auto-follow) |
| `match_drain_enabled` | **false** | **true** |

### Dlaczego match_drain różny?

**Na vast: `match_drain_enabled = false`** — vast blokuje port 80, więc `/difficulty` pada (HTTP 000). Miner używa wtedy lastblock (port 4445) jako źródła diff. Ale match-drain z lastblock=10000 == bag_target=10000 → **parkowałby GPU** myśląc że jest okno. Objaw: GPU 0-24%, found=1 na 147M hashy. **Dlatego OFF na vast.**

**Na produkcji: `match_drain_enabled = true`** — zakładamy że port 80 działa na HiveOS. Jeśli na W1-W4 też port 80 pada → ustaw `false`.

---

## Weryfikacja (co sprawdzić po starcie)

| Pole dashboardu | Oczekiwane | Znaczenie |
|-----------------|-----------|-----------|
| Network | `m=10000`+ (nie "last-good") | Auto-follow działa |
| Mining | `m=10000` (bez "Fix") | Kopie aktualny diff |
| Window | "Mining" (nie "waiting for match") | Nie parkuje |
| Speed | ~25-55 kH/s (NIE MH/s) | Normalne dla m=10000 (Argon2 wolniejszy) |
| Blocks | found rośnie **I accept rośnie** | KLUCZOWE — bloki wchodzą |

**Kryterium sukcesu:** `accept` rośnie w ciągu 1h (choćby 1-2 bloki).

---

## Wydajność — czego się spodziewać

m=10000 to 100× więcej pamięci Argon2 niż m=100 → hashrate spada drastycznie:
- **m=100 (stary):** ~2.95 MH/s
- **m=10000 (nowy):** ~25-55 kH/s

To normalne i nieuniknione (fizyka Argon2). ALE bloki teraz WCHODZĄ (accept rośnie), więc realny dochód rośnie mimo mniejszej liczby hashy.

**VRAM przy m=10000:**
- RTX 5090 (32GB): batch ~1920, dużo zapasu
- RTX 3060 (12GB): batch ~590, budżet ~5.8GB — OK, ale sprawdź że nie OOM

---

## Fallback diff (gdy /difficulty pada)

Auto-follow działa nawet bez portu 80:
1. Próba `/difficulty` (port 80)
2. Fallback: HTTPS leaderboard
3. Fallback: `lastblock` (port 4445) — osobny poller

Więc na vast (bez portu 80) miner i tak zna aktualny diff.

---

## Rozwiązywanie problemów

**GPU 0% / "waiting for match":** match_drain parkuje → ustaw `match_drain_enabled = false`.

**Speed w MH/s zamiast kH/s:** miner dalej na m=100 → sprawdź `memory_cost=10000` i `force_mine_memory_cost=0`.

**accept nie rośnie po 1h:** sprawdź czy Network pokazuje live diff (nie "last-good"); sprawdź logi submitu.

**OOM na RTX 3060:** zmniejsz `max_lanes` (np. 4) albo `parallelism`.

---

## Dashboard JSON (opcjonalny)

Jeśli chcesz webowy dashboard — zobacz `PATCH-dashboard-json.md` (dodaje eksport `data/status.json` co 4s). Wymaga edycji `src/monitoring/dashboard.cpp` + rebuild.

---

*Build: Tony.x1 & DAWIDOS.X1 · ANL Protocol · logika m=10000 przez KIMI*

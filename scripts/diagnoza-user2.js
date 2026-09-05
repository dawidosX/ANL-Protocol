// Diagnoza v2 — dla adresów z setkami pozycji (bez iteracji po indeksach).
// Uruchom w repo/scripts:  node diagnoza-user2.js 7jUhskyi82ERBCutG4TEy77F4P4VSjzQZYu73KQQyEFQ

const { Connection, PublicKey } = require("@solana/web3.js");

const RPC = "https://rpc.testnet.x1.xyz";
const PROGRAM_ID = new PublicKey("4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM");
const POSITION_LEN = 8 + 1 + 32 + 1 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 8 + 1 + 16 + 1 + 8 + 8 + 8 + 8; // 157

const u64 = (b, o) => b.readBigUInt64LE(o);
const i64 = (b, o) => b.readBigInt64LE(o);

async function main() {
  const user = new PublicKey(process.argv[2]);
  const conn = new Connection(RPC, "confirmed");

  // ── 1. INFRASTRUKTURA (najszybszy werdykt) ──
  console.log("═══ INFRASTRUKTURA ═══");
  const [capyVault] = PublicKey.findProgramAddressSync([Buffer.from("capy_vault")], PROGRAM_ID);
  const [globalCfg] = PublicKey.findProgramAddressSync([Buffer.from("global_config")], PROGRAM_ID);
  const cv = await conn.getAccountInfo(capyVault);
  console.log(`capy_vault: ${cv ? "istnieje ✅" : "❌ NIE ISTNIEJE — każdy claim pada (M-04)!"}`);
  const gc = await conn.getAccountInfo(globalCfg);
  let nowEpoch = null;
  if (gc) {
    const genesisStart = i64(gc.data, 8 + 1 + 32 * 3 + 1);
    nowEpoch = Math.floor(Date.now() / 1000 - Number(genesisStart)) / 86400 | 0;
    for (const [name, pt] of [["Genesis", 1], ["Flexible", 0]]) {
      const [pool] = PublicKey.findProgramAddressSync([Buffer.from("pool"), Buffer.from([pt])], PROGRAM_ID);
      const p = await conn.getAccountInfo(pool);
      if (!p) continue;
      const d = p.data;
      const o = 8 + 1 + 1 + 1 + 2 + 8 + 8 + 16 + 8 + 8 + 8 + 8 + 1;
      const basket = u64(d, o);
      const currentDay = u64(d, o + 8);
      const stale = Number(currentDay) < nowEpoch && basket > 0n;
      console.log(
        `pula ${name}: current_day=${currentDay} zegar=epoka ${nowEpoch} koszyk=${basket}` +
          (stale ? "  ⚠️ STALE (scenariusz M-01)" : "")
      );
    }
  }

  // ── 2. ŻYWE POZYCJE — jedno zapytanie, filtr po ownerze (offset 9) ──
  console.log("\n═══ ŻYWE POZYCJE ═══");
  const accs = await conn.getProgramAccounts(PROGRAM_ID, {
    filters: [
      { dataSize: POSITION_LEN },
      { memcmp: { offset: 9, bytes: user.toBase58() } },
    ],
  });
  console.log(`żywych pozycji: ${accs.length}`);
  const now = Math.floor(Date.now() / 1000);
  for (const { pubkey, account } of accs) {
    const b = account.data;
    let o = 8 + 1 + 32;
    const poolType = b[o]; o += 1;
    const status = b[o]; o += 1;
    const posIndex = u64(b, o); o += 8;
    const amount = u64(b, o); o += 8;
    o += 8 + 2 + 4 + 8; // shares, apy, days, start_ts
    const endTs = i64(b, o); o += 8;
    o += 8; // anl_reward
    const xntAccrued = u64(b, o); o += 8;
    const settled = b[o]; o += 1;
    o += 16 + 1; // debt, bump
    const endEpoch = u64(b, o); o += 8;
    const windowClaimed = u64(b, o);
    console.log(`\n#${posIndex} ${pubkey.toBase58()}`);
    console.log(`  ${poolType === 1 ? "Genesis" : "Flexible"} status=${status === 0 ? "Active" : "Closed"} amount=${amount}`);
    console.log(`  end_ts=${new Date(Number(endTs) * 1000).toISOString()} ${now >= endTs ? "DOJRZAŁA" : "trwa"}`);
    console.log(`  settled=${settled} accrued=${xntAccrued} window_claimed=${windowClaimed} end_epoch=${endEpoch}`);
    if (settled === 1 && xntAccrued < windowClaimed)
      console.log("  ⚠️⚠️ LOCKOUT H-01/M-01: accrued < window_claimed → claim wiecznie rewertuje");
  }

  // ── 3. BŁĘDNE TRANSAKCJE (ostatnie 25 sygnatur, tylko z err) ──
  console.log("\n═══ BŁĘDNE TRANSAKCJE ═══");
  const sigs = await conn.getSignaturesForAddress(user, { limit: 25 });
  let shown = 0;
  for (const s of sigs) {
    if (!s.err) continue;
    shown++;
    console.log(`\n❌ ${s.signature}`);
    console.log(`   ${new Date((s.blockTime ?? 0) * 1000).toISOString()}  err=${JSON.stringify(s.err)}`);
    const tx = await conn.getTransaction(s.signature, { maxSupportedTransactionVersion: 0 });
    for (const l of tx?.meta?.logMessages ?? []) {
      if (/Error|AnchorError|failed|Custom/.test(l)) console.log(`   ${l}`);
    }
    if (shown >= 6) break;
  }
  if (shown === 0) console.log("brak błędnych wśród ostatnich 25 — user może próbował dawniej / innym adresem");
}

main().catch((e) => console.error("Błąd diagnozy:", e.message));

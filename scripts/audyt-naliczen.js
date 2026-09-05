// Audytor naliczeń — porównuje stan on-chain z niezależnym przeliczeniem wg wzorów
// kontraktu (anl-math + settlement_cap_index). Tylko odczyt.
// Uruchom w repo/scripts:
//   node audyt-naliczen.js                 -> wszystkie pozycje w programie
//   node audyt-naliczen.js ADRES1 ADRES2   -> tylko podane portfele

const { Connection, PublicKey } = require("@solana/web3.js");

const RPC = "https://rpc.testnet.x1.xyz";
const P = new PublicKey("4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM");
const PREC = 10n ** 12n;
const NO_EPOCH = 18446744073709551615n;
// build testnetowy = test-periods
const W1 = 3n * 86400n, W2 = 9n * 86400n, WINDOW_DAYS = 3n;
const POSITION_LEN = 155;

const u64 = (b, o) => b.readBigUInt64LE(o);
const i64 = (b, o) => b.readBigInt64LE(o);
const u128 = (b, o) => (b.readBigUInt64LE(o + 8) << 64n) | b.readBigUInt64LE(o);
const xnt = (v) => (Number(v) / 1e9).toFixed(4);
const anl = (v) => (Number(v) / 1e9).toFixed(2);

function genesisApy(elapsed) { return elapsed < W1 ? 2000n : elapsed < W2 ? 1500n : 800n; }
function periodReward(amount, apy, days) {
  return (amount * apy * BigInt(days) * 86400n) / (10000n * 365n * 86400n);
}

async function main() {
  const conn = new Connection(RPC, "confirmed");
  const filt = new Set(process.argv.slice(2));
  const gcPk = PublicKey.findProgramAddressSync([Buffer.from("global_config")], P)[0];
  const gc = (await conn.getAccountInfo(gcPk)).data;
  const gts = i64(gc, 106);
  const now = BigInt(Math.floor(Date.now() / 1000));
  const clockEpoch = (now - gts) / 86400n;
  const progEpoch = (clockEpoch / WINDOW_DAYS) * WINDOW_DAYS - 1n;

  // pule
  const pools = {};
  for (const pt of [1, 0]) {
    const pk = PublicKey.findProgramAddressSync([Buffer.from("pool"), Buffer.from([pt])], P)[0];
    const d = (await conn.getAccountInfo(pk)).data;
    const basket = u64(d, 78), curDay = u64(d, 86);
    const rollNeeded = basket > 0n && curDay !== clockEpoch;
    const lastClosed = rollNeeded ? clockEpoch : (basket > 0n ? (curDay > 0n ? curDay - 1n : 0n) : curDay);
    pools[pt] = { shares: u64(d, 21), index: u128(d, 29), lastFunded: u64(d, 61), firstFunded: u64(d, 69), basket, curDay, lastClosed };
  }
  console.log(`zegar: epoka ${clockEpoch} | okno Genesis placi do epoki ${progEpoch}`);
  for (const pt of [1, 0]) {
    const p = pools[pt];
    console.log(`${pt === 1 ? "Genesis " : "Flexible"}: current_day=${p.curDay} koszyk=${xnt(p.basket)} last_closed=${p.lastClosed} last_funded=${p.lastFunded} shares=${anl(p.shares)}`);
  }

  // checkpointy (cache)
  const ckCache = new Map();
  async function ckpt(pt, ep) {
    const key = pt + ":" + ep;
    if (ckCache.has(key)) return ckCache.get(key);
    const eb = Buffer.alloc(8); eb.writeBigUInt64LE(ep);
    const pk = PublicKey.findProgramAddressSync([Buffer.from("xnt_ckpt"), Buffer.from([pt]), eb], P)[0];
    const a = await conn.getAccountInfo(pk);
    const v = a ? { index: u128(a.data, 18), next: u64(a.data, 34) } : null;
    ckCache.set(key, v);
    return v;
  }
  // cap_index_at: indeks ostatniego zafundowanego checkpointu <= target (jak kontrakt)
  async function capIndex(pt, target, debt) {
    const p = pools[pt];
    if (p.firstFunded === NO_EPOCH || p.firstFunded > target) return debt; // nic nie naliczono do target
    for (let e = target; e >= 0n; e--) {
      const c = await ckpt(pt, e);
      if (c) return c.index;
      if (e === 0n) break;
    }
    return debt;
  }

  // pozycje
  const accs = await conn.getProgramAccounts(P, { filters: [{ dataSize: POSITION_LEN }] });
  const byOwner = new Map();
  for (const { pubkey, account } of accs) {
    const b = account.data;
    const owner = new PublicKey(b.subarray(9, 41)).toBase58();
    if (filt.size && !filt.has(owner)) continue;
    const pos = {
      pk: pubkey.toBase58(), owner, pt: b[41], status: b[42], idx: u64(b, 43), amount: u64(b, 51),
      shares: u64(b, 59), apy: BigInt(b.readUInt16LE(67)), days: b.readUInt32LE(69), start: i64(b, 73),
      end: i64(b, 81), anlReward: u64(b, 89), xntAccrued: u64(b, 97), settled: b[105] === 1,
      debt: u128(b, 106), endEpoch: u64(b, 123), winClaimed: u64(b, 131),
    };
    if (!byOwner.has(owner)) byOwner.set(owner, []);
    byOwner.get(owner).push(pos);
  }

  const problems = [];
  for (const [owner, list] of byOwner) {
    list.sort((a, b) => Number(a.idx - b.idx));
    // profil (CAPY pending)
    const profPk = PublicKey.findProgramAddressSync([Buffer.from("profile"), new PublicKey(owner).toBuffer()], P)[0];
    const prof = await conn.getAccountInfo(profPk);
    const pendingCapy = prof ? u64(prof.data, 49) : 0n;
    console.log(`\n━━━ ${owner}  (${list.length} pozycji, CAPY pending=${anl(pendingCapy)}) ━━━`);

    for (const p of list) {
      const flags = [];
      const pool = pools[p.pt];
      const poolName = p.pt === 1 ? "Gen" : "Flex";
      // 1. APY
      const expApy = p.pt === 1 ? genesisApy(p.start - gts) : 800n;
      if (p.apy !== expApy) flags.push(`APY ${p.apy} != oczekiwane ${expApy}`);
      // 2. nagroda ANL
      const expAnl = periodReward(p.amount, p.apy, p.days);
      if (p.anlReward !== expAnl) flags.push(`ANL ${anl(p.anlReward)} != ${anl(expAnl)}`);
      // 3. end_epoch: nowa formula (pelne doby) vs stara (wlacznie z niepelna)
      const epNew = (p.end - gts) / 86400n - 1n;
      const epOld = (p.end - 1n - gts) / 86400n;
      const epNote = p.endEpoch === epNew ? "" : (p.endEpoch === epOld ? " [end_epoch wg STAREJ formuly]" : ` [end_epoch=${p.endEpoch} ≠ ${epNew}/${epOld}!]`);
      if (p.endEpoch !== epNew && p.endEpoch !== epOld) flags.push("end_epoch niezgodne z zadna formula");
      // 4. XNT
      const matured = now >= p.end;
      const target = p.endEpoch < pool.lastClosed ? p.endEpoch : pool.lastClosed;
      const cap = await capIndex(p.pt, target, p.debt);
      const expFinal = cap > p.debt ? ((cap - p.debt) * p.shares) / PREC : 0n;
      const pendingNow = pool.index > p.debt ? ((pool.index - p.debt) * p.shares) / PREC : 0n;
      let xntLine;
      if (p.settled) {
        xntLine = `XNT zamrozone=${xnt(p.xntAccrued)} (oczekiwane ${xnt(expFinal)})`;
        if (p.xntAccrued < expFinal) { flags.push(`XNT ZANIZONE o ${xnt(expFinal - p.xntAccrued)} (settle na starym programie?)`); }
        else if (p.xntAccrued > expFinal) { flags.push(`XNT zawyzone o ${xnt(p.xntAccrued - expFinal)}`); }
      } else {
        xntLine = matured ? `XNT do wyplaty=${xnt(expFinal)} (naliczone ogolem ${xnt(pendingNow)})`
                          : `XNT naliczone dotad=${xnt(pendingNow)}`;
      }
      // 5. okna Genesis
      let winLine = "";
      if (p.pt === 1 && p.status === 0) {
        const wt = progEpoch < p.endEpoch ? progEpoch : p.endEpoch;
        const wcap = wt >= 0n ? await capIndex(1, wt, p.debt) : p.debt;
        const winMax = wcap > p.debt ? ((wcap - p.debt) * p.shares) / PREC : 0n;
        const winLeft = winMax > p.winClaimed ? winMax - p.winClaimed : 0n;
        winLine = ` | okno: odebrano=${xnt(p.winClaimed)} max=${xnt(winMax)} do_odbioru=${xnt(winLeft)}`;
        if (p.winClaimed > winMax) flags.push(`okno: odebrano WIECEJ niz max o ${xnt(p.winClaimed - winMax)}`);
        if (p.settled && p.winClaimed > p.xntAccrued) flags.push("LOCKOUT: window_claimed > accrued (claim rewertuje)");
      }
      const st = p.status === 1 ? "Closed" : matured ? "MATURED" : "Active";
      console.log(`  #${p.idx} ${poolName} ${anl(p.amount)} ANL ${p.days}d apy=${p.apy} ${st}${p.settled ? " settled" : ""}${epNote}`);
      console.log(`     ANL nagroda=${anl(p.anlReward)} | ${xntLine}${winLine}`);
      if (flags.length) {
        for (const f of flags) console.log(`     ⚠️  ${f}`);
        problems.push({ owner, idx: p.idx, flags });
      }
    }
  }

  console.log(`\n═══ PODSUMOWANIE: ${byOwner.size} portfeli, ${accs.length} pozycji, ${problems.length} z flagami ═══`);
  for (const pr of problems) console.log(`${pr.owner} #${pr.idx}: ${pr.flags.join("; ")}`);
  if (!problems.length) console.log("Brak rozbieznosci — naliczenia zgodne z wzorami kontraktu.");
}

main().catch((e) => console.error("Blad:", e.message));

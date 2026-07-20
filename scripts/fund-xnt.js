// fund_xnt — zasilanie dziennej puli XNT (ANL Staking Protocol, testnet X1)
// Użycie:  node fund-xnt.js <kwota_XNT>     np.  node fund-xnt.js 0.01   (flush)
//                                                node fund-xnt.js 15     (dzienny funding)
// Podpisuje kluczem deployera (~/.config/solana/id.json).

const fs = require("fs");
const os = require("os");
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction, SystemProgram,
} = require("@solana/web3.js");

const RPC        = "https://rpc.testnet.x1.xyz";
const PROGRAM_ID = new PublicKey("6jiCawqJg5NPR26wCov15tD3HtjKVk1Ao252ZJbZYj1w");
const XNT_MINT   = new PublicKey("B9ZfZ6YuyJYGYzeZhLuKyDcsLNDjzEthAxpwtTx5dsUW");
const TOKEN_PROG = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"); // legacy SPL
const ATA_PROG   = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const DISC_FUND_XNT = Uint8Array.from([17,127,245,180,203,114,198,149]);
const NO_EPOCH = 0xFFFFFFFFFFFFFFFFn;
const DAY = 86400n;

const enc = (s) => Buffer.from(s, "utf8");
const pda = (seeds) => PublicKey.findProgramAddressSync(seeds, PROGRAM_ID)[0];
const u64le = (v) => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); return b; };

(async () => {
  const amtXnt = parseFloat(process.argv[2] || "0");
  if (!(amtXnt > 0)) { console.error("Podaj kwotę, np.: node fund-xnt.js 0.01"); process.exit(1); }
  const amount = BigInt(Math.round(amtXnt * 1e9));

  const kp = Keypair.fromSecretKey(Uint8Array.from(
    JSON.parse(fs.readFileSync(os.homedir() + "/.config/solana/id.json", "utf8"))));
  const conn = new Connection(RPC, "confirmed");
  console.log("Funder:", kp.publicKey.toBase58());

  const globalConfig   = pda([enc("global_config")]);
  const vaultAuthority = pda([enc("vault_authority")]);
  const xntVault       = pda([enc("xnt_vault")]);
  const genesisPool    = pda([enc("pool"), Buffer.from([1])]);
  const flexiblePool   = pda([enc("pool"), Buffer.from([0])]);

  // --- epoka wg zegara łańcucha (kontrakt wymaga zgodności) ---
  const gcInfo = await conn.getAccountInfo(globalConfig);
  const gs = new DataView(gcInfo.data.buffer, gcInfo.data.byteOffset).getBigInt64(106, true);
  const slot = await conn.getSlot("confirmed");
  const chainNow = BigInt(await conn.getBlockTime(slot));
  if (chainNow < gs) { console.error("Przed genesis_start_ts — coś nie tak."); process.exit(1); }
  const epoch = (chainNow - gs) / DAY;
  const secToNext = Number(DAY - ((chainNow - gs) % DAY));
  console.log("Epoka bieżąca:", epoch.toString(), `(do końca epoki ~${(secToNext/3600).toFixed(1)} h)`);
  if (secToNext < 120) { console.error("Za blisko granicy epoki (<2 min) — odpal ponownie za chwilę."); process.exit(1); }

  // --- stan pul przed ---
  const readPool = async (a) => {
    const i = await conn.getAccountInfo(a);
    const dv = new DataView(i.data.buffer, i.data.byteOffset);
    return {
      idx: (dv.getBigUint64(29 + 8, true) << 64n) | dv.getBigUint64(29, true),
      und: dv.getBigUint64(45, true),
      last: dv.getBigUint64(61, true),
    };
  };
  const gB = await readPool(genesisPool), fB = await readPool(flexiblePool);
  console.log("PRZED  Genesis : indeks", gB.idx.toString(), "| nierozdzielone", (Number(gB.und)/1e9).toFixed(4), "XNT");
  console.log("PRZED  Flexible: indeks", fB.idx.toString(), "| nierozdzielone", (Number(fB.und)/1e9).toFixed(4), "XNT");

  // --- ATA fundera i saldo ---
  const funderXnt = PublicKey.findProgramAddressSync(
    [kp.publicKey.toBuffer(), TOKEN_PROG.toBuffer(), XNT_MINT.toBuffer()], ATA_PROG)[0];
  const bal = await conn.getTokenAccountBalance(funderXnt).catch(() => null);
  if (!bal) { console.error("Brak konta XNT (SPL) u fundera:", funderXnt.toBase58()); process.exit(1); }
  console.log("Saldo XNT (SPL) fundera:", bal.value.uiAmountString);
  if (BigInt(bal.value.amount) < amount) { console.error("Za mało XNT na wpłatę", amtXnt); process.exit(1); }

  // --- checkpointy: bieżący + (opcjonalnie) poprzedni ---
  const ckpt = (poolType, ep) => pda([enc("xnt_ckpt"), Buffer.from([poolType]), u64le(ep)]);
  const genesisCkpt  = ckpt(1, epoch);
  const flexibleCkpt = ckpt(0, epoch);
  const needPrev = (last) => last !== NO_EPOCH && last !== epoch;
  const genesisPrev  = needPrev(gB.last) ? ckpt(1, gB.last) : PROGRAM_ID; // None = program id
  const flexiblePrev = needPrev(fB.last) ? ckpt(0, fB.last) : PROGRAM_ID;
  console.log("prev ckpt Genesis :", needPrev(gB.last) ? genesisPrev.toBase58() : "brak (None)");
  console.log("prev ckpt Flexible:", needPrev(fB.last) ? flexiblePrev.toBase58() : "brak (None)");

  // --- instrukcja ---
  const data = Buffer.concat([Buffer.from(DISC_FUND_XNT), u64le(amount), u64le(epoch)]);
  const AW = (pk, s=false, w=false) => ({ pubkey: pk, isSigner: s, isWritable: w });
  const keys = [
    AW(kp.publicKey, true, true),      // funder
    AW(globalConfig),                  // global_config
    AW(vaultAuthority),                // vault_authority
    AW(XNT_MINT),                      // xnt_mint
    AW(funderXnt, false, true),        // funder_xnt
    AW(xntVault, false, true),         // xnt_vault
    AW(genesisPool, false, true),      // genesis_pool
    AW(flexiblePool, false, true),     // flexible_pool
    AW(TOKEN_PROG),                    // xnt_token_program
    AW(genesisCkpt, false, true),      // genesis_ckpt
    AW(flexibleCkpt, false, true),     // flexible_ckpt
    AW(genesisPrev, false, needPrev(gB.last)),   // genesis_prev_ckpt (Option)
    AW(flexiblePrev, false, needPrev(fB.last)),  // flexible_prev_ckpt (Option)
    AW(SystemProgram.programId),       // system_program
  ];
  const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
  tx.feePayer = kp.publicKey;
  const bh = await conn.getLatestBlockhash("confirmed");
  tx.recentBlockhash = bh.blockhash;
  tx.sign(kp);
  console.log(`Wysyłam fund_xnt: ${amtXnt} XNT, epoka ${epoch}…`);
  const sig = await conn.sendRawTransaction(tx.serialize(), { skipPreflight: false });
  await conn.confirmTransaction({ signature: sig, blockhash: bh.blockhash, lastValidBlockHeight: bh.lastValidBlockHeight }, "confirmed");
  console.log("SYGNATURA:", sig);

  // --- stan po ---
  const gA = await readPool(genesisPool), fA = await readPool(flexiblePool);
  console.log("PO     Genesis : indeks", gA.idx.toString(), "| nierozdzielone", (Number(gA.und)/1e9).toFixed(4), "XNT");
  console.log("PO     Flexible: indeks", fA.idx.toString(), "| nierozdzielone", (Number(fA.und)/1e9).toFixed(4), "XNT");
  console.log(gA.und === 0n && fA.und === 0n ? "FLUSH-OK — nierozdzielone wyzerowane." : "UWAGA: nierozdzielone > 0 (pusty koszyk?).");
})().catch((e) => { console.error("BŁĄD:", e.message || e); if (e.logs) e.logs.slice(-8).forEach(l => console.error("  ", l)); process.exit(1); });

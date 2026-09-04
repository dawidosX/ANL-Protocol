import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
if 'rollNeeded' in s:
    print('JUZ ZALATANE (rollNeeded obecny) — pomijam'); sys.exit(0)
ch = []
oldA = """    let capTarget = endEpoch;
    try {
      const pI = await conn.getAccountInfo(poolConfig);
      if(pI){
        const pdv = new DataView(pI.data.buffer, pI.data.byteOffset);
        const basket = pdv.getBigUint64(78, true);
        const curDay = pdv.getBigUint64(86, true);
        const lastClosed = basket > 0n ? (curDay > 0n ? curDay - 1n : 0n) : curDay;
        if(lastClosed < endEpoch) capTarget = lastClosed;
      }
    } catch(_) {}
    const ckpt = await findCheckpoint(poolType, capTarget);"""
newA = """    let capTarget = endEpoch;
    let prevDayCkpt = null;
    try {
      const pI = await conn.getAccountInfo(poolConfig);
      const cI = await conn.getAccountInfo(globalConfig);
      if(pI && cI){
        const pdv = new DataView(pI.data.buffer, pI.data.byteOffset);
        const basket = pdv.getBigUint64(78, true);
        const curDay = pdv.getBigUint64(86, true);
        const gts = new DataView(cI.data.buffer, cI.data.byteOffset).getBigInt64(106, true);
        const clockEpoch = (BigInt(Math.floor(Date.now()/1000)) - gts) / 86400n;
        const rollNeeded = basket > 0n && curDay !== clockEpoch;
        if(rollNeeded){
          const eb = new Uint8Array(8); new DataView(eb.buffer).setBigUint64(0, curDay, true);
          prevDayCkpt = pda([enc("xnt_ckpt"), new Uint8Array([poolType]), eb]);
        }
        const lastClosed = rollNeeded ? clockEpoch
                         : (basket > 0n ? (curDay > 0n ? curDay - 1n : 0n) : curDay);
        if(lastClosed < endEpoch) capTarget = lastClosed;
      }
    } catch(_) {}
    const ckpt = await findCheckpoint(poolType, capTarget);"""
assert oldA in s, 'A nie znaleziony'
s = s.replace(oldA, newA, 1); ch.append('A')
oldB = """    if(ckpt) keys.push(AW(ckpt)); // xnt_checkpoint opcjonalny (17) — dołączamy gdy istnieje
    const tx = new Transaction();"""
newB = """    if(ckpt) keys.push(AW(ckpt));
    else if(prevDayCkpt) keys.push(AW(PROGRAM_ID));
    if(prevDayCkpt) keys.push(AW(prevDayCkpt,false,true));
    const tx = new Transaction();"""
assert oldB in s, 'B nie znaleziony'
s = s.replace(oldB, newB, 1); ch.append('B')
start = s.find('    // AUTO close_day: jesli koszyk doby <= end_epoch wisi')
assert start != -1, 'C poczatek'
endmark = '    } catch(_) {}\n'
end = s.find(endmark, start)
assert end != -1, 'C koniec'
s = s[:start] + '    // (auto close_day usuniete: program R4 domyka dobe wewnatrz claim)\n' + s[end+len(endmark):]
ch.append('C')
open(p, 'w', encoding='utf-8').write(s)
print('OK zmiany:', ','.join(ch))

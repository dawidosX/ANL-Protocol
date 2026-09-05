import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
if 'anlDialog' in s:
    print('JUZ ZALATANE'); sys.exit(0)
ch = []
css = """.anl-dlg-bg{position:fixed;inset:0;background:rgba(3,8,18,.72);backdrop-filter:blur(6px);-webkit-backdrop-filter:blur(6px);display:flex;align-items:center;justify-content:center;z-index:9999;opacity:0;transition:opacity .18s}
.anl-dlg-bg.on{opacity:1}
.anl-dlg{background:var(--card);color:var(--ink);border:1px solid var(--line);border-radius:22px;padding:26px 28px 22px;max-width:440px;width:calc(100% - 40px);box-shadow:0 24px 70px rgba(0,0,0,.55);transform:translateY(10px) scale(.98);transition:transform .18s}
.anl-dlg-bg.on .anl-dlg{transform:none}
.anl-dlg h4{margin:0 0 10px;font-family:var(--disp);font-size:1.05rem;letter-spacing:.02em;display:flex;align-items:center;gap:10px}
.anl-dlg h4 .ico{width:30px;height:30px;border-radius:10px;display:inline-flex;align-items:center;justify-content:center;font-size:1rem;background:rgba(255,177,26,.16);color:var(--gold)}
.anl-dlg.danger h4 .ico{background:rgba(255,80,80,.16);color:#ff6b6b}
.anl-dlg p{margin:0 0 20px;color:var(--mut);font-size:.95rem;line-height:1.55}
.anl-dlg .row{display:flex;gap:10px;justify-content:flex-end}
.anl-dlg .b{border:none;cursor:pointer;font-family:var(--body);font-weight:600;font-size:.9rem;border-radius:13px;padding:11px 20px;transition:transform .12s,opacity .12s}
.anl-dlg .b:hover{transform:translateY(-1px)}
.anl-dlg .b.sec{background:transparent;color:var(--mut);border:1px solid var(--line)}
.anl-dlg .b.pri{background:var(--gold);color:#0B1626}
.anl-dlg.danger .b.pri{background:#ff6b6b;color:#fff}
"""
assert '</style>' in s, 'style'
s = s.replace('</style>', css + '</style>', 1); ch.append('css')
js = """<script>
window.anlDialog = function(opts){
  opts = opts || {};
  const pl = (window.anlCurrentLang && window.anlCurrentLang()==="pl");
  const title = opts.title || (opts.danger ? (pl?"Potwierdzenie":"Confirm") : (pl?"Informacja":"Notice"));
  const okL = opts.ok || (opts.danger ? (pl?"Tak, zerwij":"Yes, unstake") : "OK");
  const ccL = opts.cancel || (pl?"Anuluj":"Cancel");
  return new Promise(function(resolve){
    const bg = document.createElement("div"); bg.className = "anl-dlg-bg";
    const box = document.createElement("div"); box.className = "anl-dlg" + (opts.danger ? " danger" : "");
    const h = document.createElement("h4");
    const ico = document.createElement("span"); ico.className = "ico"; ico.textContent = opts.danger ? "!" : "i";
    h.appendChild(ico); h.appendChild(document.createTextNode(" " + title));
    const ptxt = document.createElement("p"); ptxt.textContent = opts.text || "";
    const row = document.createElement("div"); row.className = "row";
    function close(v){ bg.classList.remove("on"); setTimeout(function(){ bg.remove(); }, 180); document.removeEventListener("keydown", onKey); resolve(v); }
    function onKey(e){ if(e.key==="Escape") close(false); if(e.key==="Enter" && !opts.danger) close(true); }
    if(!opts.alertOnly){ const c = document.createElement("button"); c.className = "b sec"; c.textContent = ccL; c.onclick = function(){ close(false); }; row.appendChild(c); }
    const ok = document.createElement("button"); ok.className = "b pri"; ok.textContent = okL; ok.onclick = function(){ close(true); }; row.appendChild(ok);
    box.appendChild(h); box.appendChild(ptxt); box.appendChild(row); bg.appendChild(box);
    bg.addEventListener("click", function(e){ if(e.target===bg) close(false); });
    document.addEventListener("keydown", onKey);
    document.body.appendChild(bg);
    requestAnimationFrame(function(){ bg.classList.add("on"); (opts.alertOnly ? ok : (opts.danger ? row.firstChild : ok)).focus(); });
  });
};
</script>
<script>
(function(){

const {Connection, PublicKey, Transaction, TransactionInstruction, SystemProgram} = solanaWeb3;"""
anchor = """<script>
(function(){

const {Connection, PublicKey, Transaction, TransactionInstruction, SystemProgram} = solanaWeb3;"""
assert s.count(anchor) == 1, 'anchor iife'
s = s.replace(anchor, js, 1); ch.append('anlDialog')
oldA = '  if(foundWallets.length===0){ alert(T("noWallet")); return; }'
newA = '  if(foundWallets.length===0){ anlDialog({text:T("noWallet"), alertOnly:true}); return; }'
assert oldA in s, 'alert'
s = s.replace(oldA, newA, 1)
oldC = '  if(!confirm(T("unstakeConfirm"))) return;'
newC = '  if(!(await anlDialog({text:T("unstakeConfirm"), danger:true}))) return;'
assert oldC in s, 'confirm'
s = s.replace(oldC, newC, 1); ch.append('alert+confirm podmienione')
open(p, 'w', encoding='utf-8').write(s)
print('OK:', ', '.join(ch))

import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
if 'anlSortPositions' in s:
    print('JUZ ZALATANE'); sys.exit(0)
ch = []
anchorA = "async function loadPositions(){"
funcs = """window.__posSort = {key:null, dir:1};
function anlRenderPositions(){
  const tb = $("positions"); const rows = (window.__posRows||[]).slice();
  const s = window.__posSort;
  if(s.key){ rows.sort(function(a,b){ return (a[s.key]-b[s.key])*s.dir || (a.idx-b.idx); }); }
  if(rows.length) tb.innerHTML = rows.map(function(r){ return r.html; }).join("");
  ["end","rank"].forEach(function(k){
    const el = document.getElementById("sa_"+k);
    if(el) el.textContent = (s.key===k) ? (s.dir===1 ? "\\u2191" : "\\u2193") : "\\u2195";
  });
}
function anlSortPositions(k){
  const key = (k===0 || k==="end") ? "end" : "rank";
  const s = window.__posSort;
  if(s.key===key) s.dir = -s.dir; else { s.key = key; s.dir = 1; }
  anlRenderPositions();
}
"""
assert s.count(anchorA) == 1, 'A: anchor'
s = s.replace(anchorA, funcs + anchorA, 1); ch.append('A funkcje')
oldB1 = """  const winProg = await genesisWindowProg();  // null = okno zamkniete
  let rows = "";"""
newB1 = """  const winProg = await genesisWindowProg();  // null = okno zamkniete
  let rows = ""; const rowObjs = [];"""
assert oldB1 in s, 'B1'
s = s.replace(oldB1, newB1, 1)
oldB2 = "    rows += `<tr><td>${i+1n}</td>"
newB2 = """    const _stRank = p.status===1 ? 2 : (now >= p.end ? 0 : 1);
    const _rowHtml = `<tr><td>${i+1n}</td>"""
assert s.count(oldB2) == 1, 'B2'
s = s.replace(oldB2, newB2, 1)
oldB3 = """<td>${st}</td></tr>`;
  }
  tb.innerHTML = rows || '<tr><td colspan="11" class="mut">'+T("noPos2")+'</td></tr>';"""
newB3 = """<td>${st}</td></tr>`;
    rowObjs.push({html:_rowHtml, end:p.end, rank:_stRank, idx:Number(i)});
  }
  window.__posRows = rowObjs;
  if(rowObjs.length) anlRenderPositions();
  else tb.innerHTML = '<tr><td colspan="11" class="mut">'+T("noPos2")+'</td></tr>';"""
assert oldB3 in s, 'B3'
s = s.replace(oldB3, newB3, 1); ch.append('B wiersze')
thE = """<th class="sortable" onclick="anlSortPositions(0)">{} <span class="sarrow" id="sa_end">\\u2195</span></th>"""
thS = """<th class="sortable" onclick="anlSortPositions(1)">Status <span class="sarrow" id="sa_rank">\\u2195</span></th></tr>'"""
assert '<th>Koniec</th>' in s and '<th>Ends</th>' in s, 'C th'
s = s.replace('<th>Koniec</th>', thE.format('Koniec'), 1)
s = s.replace('<th>Ends</th>', thE.format('Ends'), 1)
assert s.count("<th>Status</th></tr>'") == 2, 'C status x2'
s = s.replace("<th>Status</th></tr>'", thS); ch.append('C naglowki')
assert '</style>' in s, 'D style'
s = s.replace('</style>', '.sortable{cursor:pointer;user-select:none}.sortable:hover{color:#f5b942}.sarrow{opacity:.65;font-size:.85em;margin-left:2px}\n</style>', 1)
ch.append('D css')
open(p, 'w', encoding='utf-8').write(s)
print('OK:', ', '.join(ch))

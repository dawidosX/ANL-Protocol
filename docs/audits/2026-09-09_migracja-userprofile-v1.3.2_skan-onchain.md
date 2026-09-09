# Dowod migracyjny v1.3.1 -> v1.3.2: skan on-chain `UserProfile` (bajt 57)

**Cel:** v1.3.2 dodaje `UserProfile.version: u8` pod offsetem 57 (wyciete z `reserved[7]` -> `reserved[6]`, LEN 64 bez zmian)
i bramke `version <= ACCOUNT_VERSION` (= 1) w `stake` / `claim` / `claim_capy`. Konta sprzed v1.3.2 musza miec
bajt 57 == 0, inaczej bramka odrzucilaby istniejacych uzytkownikow po upgrade.

**Metoda:** `getProgramAccounts(4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM, {dataSize: 64})` na
`https://rpc.testnet.x1.xyz`, filtr dyskryminatora `sha256("account:UserProfile")[..8] = 202577cdb3b40dc2`
(jedyny typ konta programu o rozmiarze 64 B: PoolConfig 126, UserPosition 155, XntCheckpoint 56, GlobalConfig 229).
Sprawdzane: bajt 57 (przyszly `version`) oraz bajty 58..63 (przyszly `reserved`).

| Skan | Data (UTC) | Slot | Profile | bajt 57 != 0 | bajty 58..63 != 0 | max next_position_index | suma pending_capy |
|---|---|---|---|---|---|---|---|
| 1 | 2026-09-07 ~14:3x | 186387547 | 9 | 0 | 0 | 312 | 2 678 189 413 679 |
| 2 | 2026-09-09 ~07:5x | 186805680 | 9 | 0 | 0 | — | — |

**Skan 2 — lista profili (wszystkie bajt 57 = 0):**

| Profil (PDA `[profile, owner]`) | bajt 57 |
|---|---|
| 6zCGex9f4fesY2NyTsq3ToChFvT2f28NzwR8FCZm5qyT | 0 |
| QFD72n4K2v2BcBUzd5nxXZXJaJm7Rze9zvAXbLprW7B | 0 |
| 2z7WNjxF7MmfpuHzBtXNq4Bno2z5WH4htYZ6ihLyh5kS | 0 |
| 3HhHxfjpEScWFJfX8fgRCSVENuGWsKnTAeYHapM6H1fj | 0 |
| 7UfwZ38aSrZTcVWB8oKqC9m4onpqg3aRcaYEaKMU5fre | 0 |
| 4WMn3UpCi4xi2yHEYS9UEN6G3nQ6zo5azhkrTrtvRhus | 0 |
| 5gFNJ55xyLC5wzqRfezZgpXPus6JzBdGRoxkUm8VwA8g | 0 |
| 4EMFx5XyJBLrLf454jnE1JM2cyTkz36pTxHp3kidMwQM | 0 |
| DgNwSuHTfrGZoS77vRzzxWaHBQFyeE4xqswWJtCfHXNu | 0 |

**Wniosek:** wszystkie istniejace profile (9 = liczba portfeli w `audyt-naliczen.js`) maja `version` = 0 <= 1 i przechodza
bramke po upgrade. Nowe profile tworzone po v1.3.2 dostaja `version = 1`. Offsety czytane przez klientow
(`next_position_index` @40, `pending_capy` @49) bez zmian.

Skrypt skanu (python3, tylko odczyt RPC):

```python
import json,urllib.request,base64,hashlib
RPC="https://rpc.testnet.x1.xyz"; PROG="4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM"
def rpc(m,p):
    r=urllib.request.Request(RPC,data=json.dumps({"jsonrpc":"2.0","id":1,"method":m,"params":p}).encode(),headers={"content-type":"application/json"})
    return json.load(urllib.request.urlopen(r,timeout=120))["result"]
disc=hashlib.sha256(b"account:UserProfile").digest()[:8]
accs=[a for a in rpc("getProgramAccounts",[PROG,{"encoding":"base64","filters":[{"dataSize":64}]}]) if base64.b64decode(a["account"]["data"][0])[:8]==disc]
bad=[a["pubkey"] for a in accs if base64.b64decode(a["account"]["data"][0])[57]!=0]
print("slot",rpc("getSlot",[]),"profile",len(accs),"bajt57!=0",bad)
```

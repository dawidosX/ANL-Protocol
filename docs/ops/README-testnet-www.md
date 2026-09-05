# testnet.anl-protocol.com — ops

- Hosting: Hetzner (alias `ssh x1watch`), nginx vhost `sites-available/testnet.anl-protocol.com`
  (kopia: `nginx-testnet.anl-protocol.com.conf`), root `/var/www/anl-testnet`, Cloudflare przed.
- Jedyna zywa strona: `index.html` (PL/EN wbudowane, przelacznik jezyka).
  `en.html` = relikt: 301 na `/` w bloku :443 (2026-09-05), usuniety z repo; kopia na serwerze nieosiagalna.
- Deploy: `scp website/testnet/index.html x1watch:~/anl-index-new.html`, na Hetznerze
  `sudo cp` do webrootu + `chown www-data`, weryfikacja sha256 Mac == Hetzner,
  `curl | grep -c <marker>` przez Cloudflare (cf-cache-status DYNAMIC, purge zwykle zbedny).
- Backupy webrootu: `~/anl-testnet-backups/` na Hetznerze (POZA webrootem — nie serwowane).
- Patchery strony (powtarzalne, idempotentne): `patch-claim.py`, `patch-sort.py`, `patch-dialog.py`.
- Znane: nginx ostrzega o duplikatach `server_name x1watch.xyz` w innych vhostach — do posprzatania osobno.

# On-chain SBF execution evidence — split-initialize delta (testnet X1)

**Date:** 2026-07-20 · **Network:** X1 testnet (`https://rpc.testnet.x1.xyz`)
**Program ID:** `6jiCawqJg5NPR26wCov15tD3HtjKVk1Ao252ZJbZYj1w`
**Purpose:** Addresses audit finding A7-M01 (GPT, Audit #7): in-process
integration tests execute the program natively and cannot prove the SBF
stack-frame fix. The transactions below are the **compiled SBF artifact**
executing on real X1 validators — the strongest available proof that the
split-`initialize` fix works within the 4096-byte SBF frame limit.

## Build
`cargo-build-sbf --features network-testnet,test-periods`
Result: `Finished` — **zero** "overflows the maximum allowed frame space"
warnings (the pre-fix build emitted them for `Initialize`/`InitVaults`).
Artifact: `target/deploy/anl_staking.so` (523 008 B).

## Deployment
| Action | Signature |
|---|---|
| `solana program deploy` → `6jiCaw…` | `2xJiutBH27GskSqeevn3k3cLn75cyr2P5VCXQEjQhcQQKvu9PbUct8hMqYHBuRLe719Kn9tZe2WpeLk2wCa4BPNB` |

## Four-step setup executed as SBF on-chain (the audited delta)
| Instruction | Signature |
|---|---|
| `initialize` | `66x2MW1P7DNjXJC3SXyL5FX5RvjcQbtZtVPcDMCzFZordXQAAJQtGhWZU9MWCwkCfXRVqEmcaVcx3LQbY4TytojD` |
| `init_principal_vault` | `57KsPzxHXwCSkBjMUrGNbkr8XthvCzLhLUF8vDNqTYogVYbWaMPymJTYpeVfWSsyd8kbBL98xkWEu1WaeHigqyK9` |
| `init_reward_vault` | `32TvVNitcLTPzt6A4gsB3qnVqb6SeRjgNTmEc126Ji3Yw95d2TUYZGeLTGRuNvSFYmbHpqThdi71UXc5m8do8mH6` |
| `init_xnt_vault` | `2CM2m41VgQ8y3WFtoQqAAuL58ivMcpCmVBfR3rGMQe5xjQmkXtqL21riWLe8o1b6kiQ51oobZiV1Y6DerNwxkptr` |

## Post-setup lifecycle instructions executed as SBF on-chain
| Instruction | Signature |
|---|---|
| `create_pool` (Flexible) | `5ixAKH2Mvq9Faoo352TrN5FRvumMxoN7dq8WwKQJEgytebMNia1Xy4RLLrV1p9fCP7QwLa67JmBQxXBb1ysND97a` |
| `create_pool` (Genesis) | `5w4Eq1P5iEYAnuKBBmcmaQJ35VNSraqsnPd5VZCRiaqmS4x936CrLPypuap8156G2wfsWJT9TiVJ3LWjnEb6DPHb` |
| `fund_rewards` (200M ANL) | `b3cTrz6PM3dykkM95SbgAhJ8yo7E1QzXhKi11v1ryrrYQzZ2ht6ofydPN9Xt1V3CxhT8KHLa2EQch7rBh9bDadv` |
| `fund_xnt` (10 000 XNT, epoch 0) | `GV6FwQniWEE5tk3aRedEnj5KoJZaXG8svvVeeZsLFtYvkNWmsHvgokmp2oMoExzqaKWVo6BnAH5ZXU6nH22GPnt` |
| `stake` (100k ANL, Genesis) | `2XAuuMyiVFDu84aTZTJ4iCvZdNjPsp1i8ytBBHCMnTXpFeJfowpJX4Khbb17uGgKc43zfAbjzedrAzHaEnnHdo1H` |

All signatures are publicly verifiable in X1 explorers (e.g. explorer.x1.xyz,
network switched to *Testnet*), program `6jiCaw…`.

**Scope note.** Testnet Program IDs are ephemeral (environment resets); the
value of this evidence is the demonstrated execution of every new/changed
instruction of the split-`initialize` delta as a compiled SBF artifact on real
validators. Remaining A7 items (evidence run bound to a committed HEAD via
`scripts/audit-evidence.sh`) are tracked separately.

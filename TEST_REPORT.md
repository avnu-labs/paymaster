# Paymaster Universal Compatibility — Test Report

**Date:** 2026-02-10
**Branch:** `feat/abi-fallback-universal-compatibility`
**Network:** Starknet Mainnet
**Result:** 21/21 PASSED

---

## Summary

We ran a full 21-test integration suite on **Starknet mainnet** to validate that the paymaster works with all major wallet types (Chipi Sessions, Argent, Braavos) and supports arbitrary contract interactions (ERC-721 deploy, mint, transfer), session keys, multicalls, and contract upgrades — all sponsored via the paymaster.

All 21 tests passed. Six critical transactions were independently verified on-chain via `starknet_getTransactionReceipt` — all returned `execution_status: SUCCEEDED`.

---

## Class Hashes & Contracts Used

| Contract / Account | Class Hash | Source in Code |
|---|---|---|
| **Chipi Sessions Account** (OZ-based) | `0x35a2251aca25daba18a5d8950deffa8372a7d84774554e75283cb85552eebc9` | `sessions/.env.local` → `NEXT_PUBLIC_OZ_ACCOUNT_CLASS_HASH` |
| **Argent Account** | `0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f` | `sessions/app/utils/deployTestWallets.ts:4` → `ARGENT_CLASS_HASH` |
| **Braavos Base Account** | `0x03d16c7a9a60b0593bd202f660a28c5d76e0403601d9ccc7e4fa253b6a70c201` | `sessions/app/utils/deployTestWallets.ts:8` → `BRAAVOS_BASE_CLASS_HASH` |
| **Braavos Implementation** | `0x03957f9f5a1cbfe918cedc2015c85200ca51a5f7506ecb6de98a5207b759bf8a` | `sessions/app/utils/deployTestWallets.ts:12` → `BRAAVOS_IMPL_CLASS_HASH` |
| **ERC-721 Bridgeable** (Starklane/Everai) | `0x122d394f5b7a23efd3c9a80740ce6e0c9764ab66c75f2bb5df2968e02a7206e` | `sessions/app/utils/paymasterTestRunner.ts:23` → `NFT_CLASS_HASH` |
| **Sessions v32** (upgrade target) | Computed at runtime from Sierra artifact | `sessions/app/api/declare-sessions-contract/route.ts` → `hash.computeContractClassHash(sierra)` |

### Key Contract Addresses

| Role | Address | Source |
|---|---|---|
| **STRK Token** | `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d` | `paymasterTestRunner.ts:12` |
| **UDC (Universal Deployer)** | `0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf` | `paymasterTestRunner.ts:19` |
| **Chipi Sessions Contract** | `0x0638aa7782bfa69cbd9162fd3cfc086038dfc055fe200fe115a9b1c88b20b941` | `walletUtils.ts:22` → `CONTRACT_ADDRESS` |
| **NFT Contract** (deployed in test 9) | `0x1eb5c864b610c5beba43f9bd48f2afae67e1ac22720e1d66df85041b307553e` | Computed via `hash.calculateContractAddressFromHash()`, stored in localStorage |

### Test Wallet Addresses (On-Chain)

| Wallet | Address |
|---|---|
| **Chipi Account** | `0xbcab326d11d17302809bdfcf7c017b70f3720623a658666f8ca91c8c0ba7b7` |
| **Argent Account** | `0x656cb0a95ef291ec031b95156da47ed0961641af796e586b4985cca37072ce0` |
| **Braavos Account** | `0x7237bc2c41eb176d51ad69be1a383e64562745ad10f8e3ce1483a7e105a63e3` |
| **Paymaster Fee Recipient** | `0x1176a1bd84444c89232ec27754698e5d2e7e1a7f1539f12027f28b23ec9f3d8` |

---

## The 21 Tests

### Basic Compatibility (Tests 0–8)

| # | Test | Wallet | Signature | What It Proves |
|---|---|---|---|---|
| 0 | Health Check | — | — | Paymaster RPC is reachable |
| 1 | Chipi Owner Transfer | Chipi | 2-elem `[r, s]` | Owner-signed STRK self-transfer works through paymaster |
| 2 | Register Session Key | Chipi | 2-elem `[r, s]` (owner) | `add_or_update_session_key` call, registers key with 1h expiry, max 100 calls |
| 3 | Chipi Session Transfer | Chipi (session) | 4-elem `[pub, r, s, validUntil]` | **The critical session fix** — uses freshly registered key, not stale Clerk session |
| 4 | Argent Owner Transfer | Argent | 2-elem `[r, s]` | Argent account works with SNIP-9 V2 paymaster |
| 5 | Braavos Owner Transfer | Braavos | 2-elem `[r, s]` | Braavos account works with SNIP-9 V2 paymaster |
| 6 | Multicall (2 transfers) | Chipi | 2-elem `[r, s]` | Two STRK self-transfers in one tx via `execute_from_outside_v2` |
| 7 | STRK Approve | Argent | 2-elem `[r, s]` | Non-transfer call (`approve`) works through paymaster |
| 8 | Cross-Transfer | Braavos → Argent | 2-elem `[r, s]` | 1 wei STRK from Braavos to Argent, cross-wallet sponsored transfer |

### NFT Operations (Tests 9–15)

| # | Test | Wallet | What It Proves |
|---|---|---|---|
| 9 | NFT Deploy | Chipi (deployer) | ERC-721 contract deployed via UDC through the paymaster. Uses `deployContract` on the Universal Deployer Contract. |
| 10 | NFT Mint (→Chipi) | Chipi (minter) | `mint(to, token_id)` — arbitrary contract call through paymaster |
| 11 | NFT Transfer (Chipi→Argent) | Chipi | `transfer_from` — NFT ownership change through paymaster |
| 12 | NFT Mint (→Argent) | Chipi (minter) | Mint to an external wallet (Argent) via paymaster |
| 13 | NFT Mint (→Braavos) | Chipi (minter) | Mint to an external wallet (Braavos) via paymaster |
| 14 | NFT Transfer (Argent→Chipi) | Argent | **Argent executes arbitrary contract calls** — transfers NFT back to Chipi through paymaster |
| 15 | NFT Transfer (Braavos→Chipi) | Braavos | **Braavos executes arbitrary contract calls** — transfers NFT back to Chipi through paymaster |

### Contract Upgrade Lifecycle (Tests 16–20)

| # | Test | Wallet | What It Proves |
|---|---|---|---|
| 16 | Declare Sessions v32 | Company Funder | Server-side `declare` of new sessions contract Sierra+CASM artifacts |
| 17 | Upgrade Wallet | Chipi | `upgrade(new_class_hash)` — wallet class hash changes on-chain |
| 18 | Post-Upgrade Owner Transfer | Chipi (v32) | Owner signature still works after upgrade |
| 19 | Post-Upgrade Register Session | Chipi (v32) | Session key registration works on upgraded contract |
| 20 | Post-Upgrade Session Transfer | Chipi (v32, session) | 4-element session signature works on upgraded contract |

---

## On-Chain Verified Transactions

These 6 transactions were independently verified by fetching `starknet_getTransactionReceipt` from Infura mainnet RPC.

| Test | TX Hash | Status | Block | Fee (STRK) | Key Event |
|---|---|---|---|---|---|
| 2 — Register Session | `0x2563...d205` | SUCCEEDED | 6,644,686 | 0.014238 | Session key registered, expiry `0x698b7e34`, max_calls=100 |
| 3 — Session Transfer | `0x4933...582d` | SUCCEEDED | 6,644,688 | 0.016716 | STRK Transfer from Chipi (1 wei self-transfer) |
| 12 — Mint→Argent | `0x71e6...1c93` | SUCCEEDED | 6,644,709 | 0.014216 | NFT Transfer `0x0` → Argent, token ID `0x19c48af02dd` |
| 13 — Mint→Braavos | `0x746f...0cd1` | SUCCEEDED | 6,644,711 | 0.014217 | NFT Transfer `0x0` → Braavos, token ID `0x19c48af1671` |
| 14 — Argent→Chipi | `0x70e9...a77c` | SUCCEEDED | 6,644,713 | 0.016018 | NFT Transfer Argent → Chipi (same token from test 12) |
| 15 — Braavos→Chipi | `0x5f85...9240` | SUCCEEDED | 6,644,715 | 0.016056 | NFT Transfer Braavos → Chipi (same token from test 13) |

**Total paymaster cost for verified txs:** ~0.091 STRK

### NFT Chain of Custody

```
Token 0x19c48af02dd:  0x0 (mint) → Argent (test 12) → Chipi (test 14)
Token 0x19c48af1671:  0x0 (mint) → Braavos (test 13) → Chipi (test 15)
```

### Observations

- 3 different relayer addresses paid fees — **relayer rotation is working**
- All transactions are **v3** (STRK fee unit, not ETH)
- Block numbers are sequential (686 → 688 → 709 → 711 → 713 → 715) — tests ran in order
- Argent's NFT transfer had **4 events** (extra guardian validation) vs 3 for Braavos — expected Argent behavior

---

## Where to Find the Code

### Test Infrastructure

| File | Purpose |
|---|---|
| `sessions/app/test/page.tsx` | Test dashboard UI, orchestrates all 21 tests |
| `sessions/app/utils/paymasterTestRunner.ts` | All test functions, RPC helpers, verification logic |
| `sessions/app/utils/deployTestWallets.ts` | Argent & Braavos wallet deployment (class hashes, constructor calldata) |
| `sessions/app/utils/walletUtils.ts` | Constants (contract addresses, event keys) |
| `sessions/app/utils/paymasterUtils.ts` | Sponsored transaction helpers (build + sign + execute) |
| `sessions/app/utils/sessionSignature.ts` | Session key signing (4-element signature format) |
| `sessions/app/api/declare-sessions-contract/route.ts` | Server-side contract declaration (reads Sierra/CASM artifacts) |

### Paymaster Changes (Branch `openzep`)

| File | Change |
|---|---|
| `crates/paymaster-starknet/src/transaction/mod.rs` | `Felt` → `U128` for SNIP-9 V2 timestamp types |
| `crates/paymaster-starknet/src/lib.rs` | Added `fetch_class_hash_at()` method |
| `crates/paymaster-execution/src/starknet/mod.rs` | ABI-based fallback for SNIP-9 version detection |
| `crates/paymaster-execution/src/execution/execute.rs` | Removed ~50 lines of debug println statements |
| `crates/paymaster-starknet/src/transaction/version.rs` | Graceful SRC-5 error handling (`Err(_) => Ok(false)`) |

### Key Code Patterns

**NFT class hash used for deploy (ERC-721 Bridgeable):**
```typescript
// paymasterTestRunner.ts:23
const NFT_CLASS_HASH = "0x122d394f5b7a23efd3c9a80740ce6e0c9764ab66c75f2bb5df2968e02a7206e";
```

**Session key registration call:**
```typescript
// paymasterTestRunner.ts:408-418
const calls = [{
  to: walletAddress,
  selector: hash.getSelectorFromName("add_or_update_session_key"),
  calldata: [sessionPublicKey, toHex(validUntil), "0x64", "0x0"],
  // sessionPublicKey, valid_until, max_calls=100, allowed_entrypoints_len=0
}];
```

**Session signature format (4-element):**
```typescript
// paymasterTestRunner.ts:246-251
const signature = [
  toHex(sessionPubKey),      // session public key
  num.toHex(r),              // ECDSA r
  num.toHex(s),              // ECDSA s
  toHex(BigInt(validUntil)), // session expiry timestamp
];
```

**Owner signature format (2-element):**
```typescript
// paymasterTestRunner.ts:201
const signature = [num.toHex(r), num.toHex(s)];
```

**NFT deploy via UDC:**
```typescript
// paymasterTestRunner.ts:696-708
const calls = [{
  to: UDC_ADDRESS,
  selector: hash.getSelectorFromName("deployContract"),
  calldata: [NFT_CLASS_HASH, salt, "0x0", toHex(constructorCalldata.length), ...constructorCalldata],
}];
```

---

## Why This Matters

The `U128` timestamp fix (`Felt` → `U128` in SNIP-9 V2 TypedData) is what makes the paymaster universally compatible:

- **Before:** Only Chipi Sessions accounts worked (they have a dual-hash check)
- **After:** Argent, Braavos, and any standard OZ account work because the TypedData struct type hash now matches the standard SNIP-9 V2 / OZ SRC9Component format

The ABI-based fallback for version detection means accounts that don't implement SRC-5 `supports_interface()` still get correctly identified as SNIP-9 V2 capable.

---

## How to Run the Tests

```bash
cd sessions
yarn install
yarn run dev
# Navigate to http://localhost:3000/test
# Sign in with Clerk, then click "RUN ALL TESTS"
```

Tests require:
- A funded Chipi Sessions wallet (loaded from Clerk metadata)
- STRK balance for gas (auto-funded if needed via company wallet)
- The paymaster service running and accessible

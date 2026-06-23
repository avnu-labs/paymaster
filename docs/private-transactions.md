# Private Transactions

The paymaster supports private transactions through privacy pool integration ([#67](https://github.com/avnu-labs/paymaster/pull/67)). Users can deposit, withdraw, and transact through a privacy pool while paying fees gaslessly or with sponsorship.

Two transaction types support private flows:

- **`apply_action`** — a private action on the pool with no user calls (e.g. a withdraw)
- **`invoke_and_apply_action`** — a private action plus user calls executed via `execute_from_outside` (e.g. an approve before a deposit)

On-chain, the paymaster wraps everything into a single transaction via the forwarder's `execute_private` / `execute_private_sponsored` entrypoints (see [contracts/README.md](../contracts/README.md)).

## Fee Modes

| Mode | Gas fee | Pool fee | Fee token |
|---|---|---|---|
| **`sponsored`** | Paid by relayer | Paid by user from private balance | Always **STRK** (hardcoded) |
| **`sponsored_private`** (recommended) | Paid by relayer | Paid by user from private balance | User's choice via `fee_mode.pool_fee_token` |
| **`default`** (gasless) | Paid by user from private balance | Paid by user from private balance | User's choice via `fee_mode.gas_token` |

- In **`sponsored`** mode, the `fee_action` returned by `buildTransaction` always uses STRK as the token. The pool fee amount is a fixed server-side configuration (`privacy.pool_fee_amount`).
- In **`sponsored_private`** mode, the relayer pays gas (same as sponsored), but the user chooses which token to pay the pool fee in via `pool_fee_token` (ETH, USDC, STRK…). The pool fee amount is converted from the base STRK amount to the chosen token using the price oracle. This mode is **only valid for private transaction types** (`apply_action` / `invoke_and_apply_action`) — using it with `deploy`, `invoke`, or `deploy_and_invoke` returns error **168** (`SPONSORED_PRIVATE_REQUIRES_PRIVACY`).
- In **`default`** (gasless) mode, the user chooses the fee token (STRK, USDC, ETH…) and pays both gas + pool fee from their private balance in that token.

All fee modes accept an optional `tip` priority: `slow`, `normal`, or `fast`.

## Sponsored Private Transaction Flow

Two transaction types depending on whether user calls (e.g. approve) are needed:

### `apply_action` — no user call needed (e.g. withdraw)

```
┌────────┐                    ┌───────────┐                  ┌──────────────┐
│ Wallet │                    │ Paymaster │                  │ Proving Svc  │
└───┬────┘                    └─────┬─────┘                  └──────┬───────┘
    │                               │                               │
    │  1. buildTransaction          │                               │
    │  { type: "apply_action",      │                               │
    │    apply_action: { pool } }   │                               │
    │  fee_mode: { mode:            │                               │
    │    "sponsored_private",       │                               │
    │    pool_fee_token: ETH,       │                               │
    │    tip: "normal" }            │                               │
    │──────────────────────────────>│                               │
    │                               │                               │
    │  fee_action: { recipient,     │                               │
    │    token: ETH,                │                               │
    │    amount: pool_fee_in_eth }  │                               │
    │<──────────────────────────────│                               │
    │                               │                               │
    │  2. Build proof: user action  │                               │
    │     + withdraw (pool fee      │                               │
    │     to forwarder in ETH)      │                               │
    │──────────────────────────────────────────────────────────────>│
    │                               │                   proof + call│
    │<──────────────────────────────────────────────────────────────│
    │                               │                               │
    │  3. executeTransaction        │                               │
    │  { type: "apply_action",      │                               │
    │    apply_action: { call,      │                               │
    │      proof, proof_facts } }   │                               │
    │──────────────────────────────>│                               │
    │                               │──> forwarder                  │
    │                               │    .execute_private_sponsored(│
    │                               │      [apply_actions],         │
    │                               │      ETH, pool_fee,           │
    │                               │      sponsor_metadata)        │
    │  { transaction_hash }         │                               │
    │<──────────────────────────────│                               │
```

### `invoke_and_apply_action` — with user calls (e.g. approve for deposit)

```
┌────────┐                    ┌───────────┐                  ┌──────────────┐
│ Wallet │                    │ Paymaster │                  │ Proving Svc  │
└───┬────┘                    └─────┬─────┘                  └──────┬───────┘
    │                               │                               │
    │  1. buildTransaction          │                               │
    │  { type:                      │                               │
    │    "invoke_and_apply_action", │                               │
    │    invoke: { user, calls:     │                               │
    │      [approve] },             │                               │
    │    apply_action: { pool } }   │                               │
    │  fee_mode: { mode:            │                               │
    │    "sponsored_private",       │                               │
    │    pool_fee_token: ETH,       │                               │
    │    tip: "normal" }            │                               │
    │──────────────────────────────>│                               │
    │                               │                               │
    │  typed_data (approve via      │                               │
    │    execute_from_outside)      │                               │
    │  + fee_action: { recipient,   │                               │
    │    token: ETH,                │                               │
    │    amount: pool_fee_in_eth }  │                               │
    │<──────────────────────────────│                               │
    │                               │                               │
    │  2. Build proof: user action  │                               │
    │     + withdraw (pool fee      │                               │
    │     to forwarder in ETH)      │                               │
    │──────────────────────────────────────────────────────────────>│
    │                               │                   proof + call│
    │<──────────────────────────────────────────────────────────────│
    │                               │                               │
    │  3. Sign typed_data (approve) │                               │
    │                               │                               │
    │  4. executeTransaction        │                               │
    │  { type:                      │                               │
    │    "invoke_and_apply_action", │                               │
    │    invoke: { user,            │                               │
    │      typed_data, signature }, │                               │
    │    apply_action: { call,      │                               │
    │      proof, proof_facts } }   │                               │
    │──────────────────────────────>│                               │
    │                               │──> forwarder                  │
    │                               │    .execute_private_sponsored(│
    │                               │      [efo(approve),           │
    │                               │       apply_actions],         │
    │                               │      ETH, pool_fee,           │
    │                               │      sponsor_metadata)        │
    │  { transaction_hash }         │                               │
    │<──────────────────────────────│                               │
```

> **Note:** The `sponsored` mode (without `_private`) works identically but always uses STRK as the pool fee token. Replace `"sponsored_private"` with `"sponsored"` and remove `pool_fee_token` to use it.

## Wallet Integration Guide

### 1. `buildTransaction`

#### Sponsored mode (`sponsored`)

The wallet calls `buildTransaction` with `fee_mode: { mode: "sponsored", tip }`. The sponsor (relayer) covers the gas fee. The user pays the **pool fee** from their private balance in **STRK**.

#### Sponsored Private mode (`sponsored_private`)

The wallet calls `buildTransaction` with `fee_mode: { mode: "sponsored_private", pool_fee_token: "<token_address>", tip }`. The sponsor (relayer) covers the gas fee. The user pays the **pool fee** from their private balance in the **chosen token** (ETH, USDC, STRK…). The paymaster converts the base pool fee amount to the chosen token via the price oracle.

> **Important:** `sponsored_private` is only valid for private transaction types (`apply_action` / `invoke_and_apply_action`). Using it with `deploy`, `invoke`, or `deploy_and_invoke` returns error **168** (`SPONSORED_PRIVATE_REQUIRES_PRIVACY`).

#### Response

The response contains:

- **`fee_action`**: a Withdraw action the wallet must include in the proof. In sponsored / sponsored_private mode, this covers only the pool fee (not gas).
  - `fee_action.recipient` — the forwarder address
  - `fee_action.token` — STRK for `sponsored`, user-chosen token for `sponsored_private`
  - `fee_action.amount` — the pool fee amount (converted to the chosen token for `sponsored_private`)
- **`typed_data`** (only for `invoke_and_apply_action`): an `execute_from_outside` message wrapping the user calls (e.g. approve). The wallet must ask the user to sign it.

> **Note:** If `fee_action.amount` is `0x0`, the pool fee is zero and the wallet can skip the fee withdraw in the proof.

### 2. Build the proof

The wallet builds the proof using the privacy SDK. The `fee_action` returned by the paymaster must be added as the last withdraw in the proof's action list:

```ts
// sponsored — pool fee always in STRK
transfers.build().with(STRK, (t) =>
  t.deposit({ amount })
   .withdraw({
     recipient: build.fee_action.recipient,
     amount: build.fee_action.amount,
   })
)

// sponsored_private — pool fee in the chosen token (e.g. ETH)
transfers.build().with(ETH, (t) =>
  t.deposit({ amount })
   .withdraw({
     recipient: build.fee_action.recipient,
     amount: build.fee_action.amount,
   })
)
```

### 3. `executeTransaction`

The wallet sends the proof + call to `executeTransaction`. For `invoke_and_apply_action`, the signed `typed_data` + `signature` must also be provided in the `invoke` field.

The paymaster wraps everything into a single on-chain transaction via the forwarder's `execute_private_sponsored` entrypoint. The relayer pays gas, the forwarder collects the pool fee from the `TransferTo` action in the proof.

## Code Snippets

### Sponsored Deposit (STRK pool fee)

```ts
// 1. Build — server returns typed_data (for approve) + fee info
const build = await paymaster.buildTransaction({
  transaction: {
    type: "invoke_and_apply_action",
    invoke: {
      user_address: account.address,
      calls: [{ to: TOKEN, selector: "approve", calldata: [POOL, amount, "0x0"] }],
    },
    apply_action: { pool_address: POOL },
  },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", tip: "normal" } },
});

// 2. Generate proof: deposit + withdraw pool fee (STRK) from private balance
const { call, proof } = await transfers
  .build()
  .with(TOKEN, (t) => t.deposit({ amount }))
  .with(STRK, (t) =>
    t.withdraw({ recipient: build.fee_action.recipient, amount: build.fee_action.amount })
  )
  .execute({ provingBlockId });

// 3. Sign the approve typed_data & execute
const signature = await account.signMessage(build.typed_data);

await paymaster.executeTransaction({
  transaction: {
    type: "invoke_and_apply_action",
    invoke: { user_address: account.address, typed_data: build.typed_data, signature },
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", tip: "normal" } },
});
```

### Sponsored Private Deposit (user-chosen pool fee token)

```ts
const ETH = "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7";

// 1. Build — server returns typed_data (for approve) + fee info in ETH
const build = await paymaster.buildTransaction({
  transaction: {
    type: "invoke_and_apply_action",
    invoke: {
      user_address: account.address,
      calls: [{ to: ETH, selector: "approve", calldata: [POOL, amount, "0x0"] }],
    },
    apply_action: { pool_address: POOL },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: ETH, tip: "normal" },
  },
});

// 2. Generate proof: deposit ETH + withdraw pool fee (ETH) from private balance
//    Both operations are on the same token, so they can be chained in a single .with()
const { call, proof } = await transfers
  .build()
  .with(ETH, (t) =>
    t.deposit({ amount })
     .withdraw({ recipient: build.fee_action.recipient, amount: build.fee_action.amount })
  )
  .execute({ provingBlockId });

// 3. Sign the approve typed_data & execute
const signature = await account.signMessage(build.typed_data);

await paymaster.executeTransaction({
  transaction: {
    type: "invoke_and_apply_action",
    invoke: { user_address: account.address, typed_data: build.typed_data, signature },
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: ETH, tip: "normal" },
  },
});
```

### Sponsored Withdraw

```ts
// 1. Build — server returns fee info (no typed_data needed, no user call)
const build = await paymaster.buildTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { pool_address: POOL },
  },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", tip: "normal" } },
});

// 2. Generate proof: withdraw to user + withdraw pool fee (STRK) from private balance
const { call, proof } = await transfers
  .build()
  .with(TOKEN, (t) =>
    t.withdraw({ recipient: account.address, amount: withdrawAmount })
  )
  .with(STRK, (t) =>
    t.withdraw({ recipient: build.fee_action.recipient, amount: build.fee_action.amount })
  )
  .execute({ provingBlockId });

// 3. Execute — no signature needed, everything is on-chain
await paymaster.executeTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", tip: "normal" } },
});
```

### Sponsored Private Withdraw (user-chosen pool fee token)

```ts
const ETH = "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7";

// 1. Build — server returns fee info in ETH
const build = await paymaster.buildTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { pool_address: POOL },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: ETH, tip: "normal" },
  },
});

// 2. Generate proof: withdraw TOKEN to user + withdraw pool fee (ETH) from private balance
const { call, proof } = await transfers
  .build()
  .with(TOKEN, (t) =>
    t.withdraw({ recipient: account.address, amount: withdrawAmount })
  )
  .with(ETH, (t) =>
    t.withdraw({ recipient: build.fee_action.recipient, amount: build.fee_action.amount })
  )
  .execute({ provingBlockId });

// 3. Execute
await paymaster.executeTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: ETH, tip: "normal" },
  },
});
```

### Sponsored Private Swap via avnu

> **Note:** This flow requires `@avnu/avnu-sdk` ≥ `4.1.0-next.2`. As of June 2026 this version is not yet published to npm — published versions (up to `4.1.0-next.1`) do not expose the `private` option in `quoteToCalls`, the `executorAddress` return value, or `serializeCalls`.

```ts
import { getQuotes, quoteToCalls } from "@avnu/avnu-sdk";

// 1. Get quote + build swap calls with private: true
//    Backend automatically sets takerAddress = executor, returns inner calls + executorAddress
const [quote] = await getQuotes({ sellTokenAddress: STRK, buyTokenAddress: ETH, sellAmount, takerAddress: account.address, size: 1 });
const { calls, executorAddress } = await quoteToCalls({ quoteId: quote.quoteId, slippage: 0.05, private: true });

// 2. Build — server returns fee info
const build = await paymaster.buildTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { pool_address: POOL },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: STRK, tip: "normal" },
  },
});

// 3. Generate proof: withdraw sell token to executor + fee + open note for buy token
const { call, proof } = await transfers
  .build()
  .with(STRK, (t) => {
    t.withdraw({ recipient: executorAddress, amount: sellAmount });
    t.withdraw({ recipient: build.fee_action.recipient, amount: build.fee_action.amount });
    t.surplusTo(account.address);
  })
  .with(ETH, (t) => t.transfer({ recipient: account.address, amount: Open }))
  .invoke(({ openNotes }) => ({
    contractAddress: executorAddress,
    calldata: [ETH, ...serializeCalls(calls), openNotes[0].noteId],
  }))
  .execute({ provingBlockId });

// 4. Execute
await paymaster.executeTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: {
    version: "0x1",
    fee_mode: { mode: "sponsored_private", pool_fee_token: STRK, tip: "normal" },
  },
});
```

## Server Configuration

Private transaction support is configured under the `privacy` section of the service configuration:

```json
{
  "privacy": {
    "pool": "0x...",
    "pool_fee_amount": "1000000000000000",
    "gas_overhead": 1000000
  }
}
```

| Field | Description |
|---|---|
| `pool` | Address of the privacy pool contract |
| `pool_fee_amount` | Pool's `collect_fee` cost in STRK (decimal string). This is the base amount converted to the chosen token in `sponsored_private` mode |
| `gas_overhead` | L2 gas overhead for privacy pool execution (proof verification, forwarder, etc.). Used at build time to estimate fees before the proof is available |

## See Also

- [OpenRPC specification](specification/paymaster.openrpc.json) — full schema for `apply_action`, `invoke_and_apply_action`, `fee_mode`, and `fee_action`
- [Forwarder contract](../contracts/README.md) — `execute_private` / `execute_private_sponsored` entrypoints

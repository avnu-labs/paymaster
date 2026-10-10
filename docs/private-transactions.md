# Private Transactions

The paymaster supports private transactions through privacy pool integration ([#67](https://github.com/avnu-labs/paymaster/pull/67)). Users can deposit, withdraw, and transact through a privacy pool while paying fees gaslessly or with sponsorship.

Two transaction types support private flows:

- **`apply_action`** — a private action on the pool with no user calls (e.g. a withdraw)
- **`invoke_and_apply_action`** — a private action plus user calls executed via `execute_from_outside` (e.g. an approve before a deposit)

On-chain, the paymaster wraps everything into a single transaction via the forwarder's `execute_private` / `execute_private_sponsored` entrypoints (see [contracts/README.md](../contracts/README.md)).

## Fee Modes

A private transaction has two fees: the **gas fee** of the Starknet transaction, and the **pool fee** charged by the privacy pool on every `apply_actions` call. `fee_mode` says who pays each one: `mode` is about the gas fee, `pool_fee` is about the pool fee.

| `fee_mode` | Gas fee | Pool fee | Fee token |
|---|---|---|---|
| `{ mode: "sponsored", pool_fee: { paid_by: "user", token } }` | Sponsor | User, from private balance | `pool_fee.token` (ETH, USDC, STRK…) |
| `{ mode: "sponsored", pool_fee: { paid_by: "sponsor" } }` | Sponsor | Sponsor | None: the user pays nothing |
| `{ mode: "default", gas_token }` (gasless) | User, from private balance | User, from private balance | `gas_token` |

- **`pool_fee: { paid_by: "user", token }`**: the sponsor (relayer) pays gas and the user pays the pool fee from their private balance in the chosen token. `buildTransaction` returns a `fee_action` of type `withdraw` the wallet must add to the proof. The pool fee amount is a fixed server-side configuration (`privacy.pool_fee_amount`), converted from STRK to the chosen token using the price oracle.
- **`pool_fee: { paid_by: "sponsor" }`**: the sponsor pays both gas and the pool fee. `buildTransaction` returns a `fee_action` of type `none` and the proof must not contain a fee withdrawal, so a user holding no shielded balance at all can transact (e.g. register with the pool). The API key must be allowed to sponsor the pool fee, see [Pool fee paid by the sponsor](#pool-fee-paid-by-the-sponsor).
- **`default`** (gasless): the user chooses the fee token (STRK, USDC, ETH…) and pays both gas + pool fee from their private balance in that token.

On private transaction types (`apply_action` / `invoke_and_apply_action`), `pool_fee` is **required** in `sponsored` mode: `{ mode: "sponsored" }` alone returns error **171** (`POOL_FEE_REQUIRED`). On any other transaction type there is no pool fee, so `pool_fee` must be omitted: sending it returns error **170** (`POOL_FEE_REQUIRES_PRIVACY`).

All fee modes accept an optional `tip` priority: `slow`, `normal`, or `fast`.

> **Deprecated:** `{ mode: "sponsored_private", pool_fee_token }` is the former spelling of `{ mode: "sponsored", pool_fee: { paid_by: "user", token } }`. It is still accepted, behaves as before, and is echoed back unchanged, so existing integrations keep working. New integrations should use `pool_fee`.

### Pool fee paid by the sponsor

With `pool_fee: { paid_by: "sponsor" }` the relayer pays gas and the pool fee, and the user pays nothing:

- `buildTransaction` returns `fee_action: { type: "none" }`. The wallet must not add a fee withdrawal to the proof.
- `executeTransaction` accepts proofs without a fee withdrawal. The forwarder is called with a zero gas amount: it still approves the pool fee pre-transferred by the relayer, but expects nothing from the user. A proof that contains a fee withdrawal to the forwarder anyway is rejected with error **173** (`UNEXPECTED_FEE_TRANSFER_TO`): the forwarder would not collect it.
- This makes **relayed registration** possible: a proof containing only setup actions (set viewing key, open channels/subchannels) and no notes can be relayed for a user holding no shielded balance.
- The `fee` estimate still reports gas plus the pool fee, so the sponsoring backend can see what each request costs.

The sponsor bears the pool fee, so its backend must allow it for the API key. The permission is off by default:

| Sponsoring mode | How to allow `pool_fee: { paid_by: "sponsor" }` |
|---|---|
| `webhook` | Return `"allow_pool_fee_sponsoring": true` in the API key validation response (optional, defaults to `false`) |
| `self` | Set `"allow_pool_fee_sponsoring": true` in the self-sponsoring configuration (optional, defaults to `false`) |

A key without the permission that requests `paid_by: "sponsor"` gets error **172** (`POOL_FEE_SPONSORING_NOT_ALLOWED`). It can still use `paid_by: "user"`.

Billing the sponsor for the gas and pool fee it covers is the sponsoring backend's responsibility. In `self` sponsoring mode there is no backend to enforce a budget: every request authenticated with the configured key is sponsored, pool fee included. Only enable `allow_pool_fee_sponsoring` there for keys you fully control.

> **Privacy note:** a transaction whose pool fee is paid by the sponsor has no public `pool → forwarder` fee withdrawal, which distinguishes it on-chain from transactions where the user pays the pool fee.

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
    │    "sponsored",               │                               │
    │    pool_fee: { paid_by: user, │                               │
    │      token: ETH },            │                               │
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
    │    "sponsored",               │                               │
    │    pool_fee: { paid_by: user, │                               │
    │      token: ETH },            │                               │
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

> **Note:** With `pool_fee: { paid_by: "sponsor" }` the flow is the same, except that `fee_action` is `{ type: "none" }` and the proof contains no fee withdrawal.

## Wallet Integration Guide

### 1. `buildTransaction`

#### Pool fee paid by the user

The wallet calls `buildTransaction` with `fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: "<token_address>" }, tip }`. The sponsor (relayer) covers the gas fee. The user pays the **pool fee** from their private balance in the **chosen token** (ETH, USDC, STRK…). The paymaster converts the base pool fee amount to the chosen token via the price oracle.

#### Pool fee paid by the sponsor

The wallet calls `buildTransaction` with `fee_mode: { mode: "sponsored", pool_fee: { paid_by: "sponsor" }, tip }`. The sponsor covers both the gas fee and the pool fee. `fee_action` is `{ type: "none" }` and the proof must not contain a fee withdrawal. The API key must be allowed to sponsor the pool fee (see [Pool fee paid by the sponsor](#pool-fee-paid-by-the-sponsor)).

> **Important:** `pool_fee` is required on private transaction types (`apply_action` / `invoke_and_apply_action`) and must be omitted on `deploy`, `invoke`, or `deploy_and_invoke`. See [Fee Modes](#fee-modes) for the error codes.

#### Response

The response contains:

- **`fee_action`**: what the wallet must include in the proof to pay the fees, tagged by `type`:
  - `{ type: "withdraw", recipient, token, amount }` when the user pays the pool fee. In sponsored mode this covers only the pool fee (not gas). `recipient` is the forwarder address, `token` is `pool_fee.token`, `amount` is the pool fee converted to that token.
  - `{ type: "none" }` when the sponsor pays the pool fee: the wallet must not add a fee withdraw to the proof.
- **`typed_data`** (only for `invoke_and_apply_action`): an `execute_from_outside` message wrapping the user calls (e.g. approve). The wallet must ask the user to sign it.

> **Note:** If `fee_action.amount` is `0x0`, the pool fee is zero and the wallet can skip the fee withdraw in the proof.

### 2. Build the proof

The wallet builds the proof using the privacy SDK. A `fee_action` of type `withdraw` must be added as the last withdraw in the proof's action list:

```ts
// pool_fee: { paid_by: "user", token: STRK } — pool fee in STRK
transfers.build().with(STRK, (t) =>
  t.deposit({ amount })
   .withdraw({
     recipient: build.fee_action.recipient,
     amount: build.fee_action.amount,
   })
)

// pool_fee: { paid_by: "user", token: ETH } — pool fee in ETH
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

The paymaster wraps everything into a single on-chain transaction via the forwarder's `execute_private_sponsored` entrypoint. The relayer pays gas, the forwarder collects the pool fee from the `TransferTo` action in the proof. When the sponsor pays the pool fee there is no `TransferTo` action and the forwarder collects nothing from the user.

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
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" } },
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
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" } },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: ETH }, tip: "normal" },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: ETH }, tip: "normal" },
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
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" } },
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
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" } },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: ETH }, tip: "normal" },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: ETH }, tip: "normal" },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" },
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
    fee_mode: { mode: "sponsored", pool_fee: { paid_by: "user", token: STRK }, tip: "normal" },
  },
});
```

### Registration with the pool fee paid by the sponsor

```ts
// 1. Build — fee_action is { type: "none" } when the sponsor pays the pool fee
const build = await paymaster.buildTransaction({
  transaction: { type: "apply_action", apply_action: { pool_address: POOL } },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "sponsor" }, tip: "normal" } },
});
// build.fee_action.type === "none"

// 2. Generate a proof with setup actions only (viewing key, channels), no notes and no fee withdraw
const { call, proof } = await registration.execute({ provingBlockId });

// 3. Execute — the relayer pays gas and the pool fee
await paymaster.executeTransaction({
  transaction: {
    type: "apply_action",
    apply_action: { apply_actions_call: call, proof: proof.data, proof_facts: proof.proofFacts },
  },
  parameters: { version: "0x1", fee_mode: { mode: "sponsored", pool_fee: { paid_by: "sponsor" }, tip: "normal" } },
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
| `pool_fee_amount` | Pool's `collect_fee` cost in STRK (decimal string). This is the base amount converted to `pool_fee.token` when the user pays the pool fee |
| `gas_overhead` | L2 gas overhead for privacy pool execution (proof verification, forwarder, etc.). Used at build time to estimate fees before the proof is available |

Letting the sponsor pay the pool fee is allowed per API key through the `sponsoring` section: `allow_pool_fee_sponsoring` in the `self` configuration, or `allow_pool_fee_sponsoring` in the `webhook` validation response (see [Pool fee paid by the sponsor](#pool-fee-paid-by-the-sponsor)).

## See Also

- [OpenRPC specification](specification/paymaster.openrpc.json) — full schema for `apply_action`, `invoke_and_apply_action`, `fee_mode`, and `fee_action`
- [Forwarder contract](../contracts/README.md) — `execute_private` / `execute_private_sponsored` entrypoints

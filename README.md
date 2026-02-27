# avnu Paymaster

Gas abstraction made easy on Starknet — open-source, production-ready, fully extensible.

[![License: AGPL v3](https://img.shields.io/badge/license-AGPLv3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Docs](https://img.shields.io/badge/docs-available-green)](https://docs.avnu.fi/docs/paymaster)
[![Build](https://img.shields.io/github/actions/workflow/status/avnu-labs/paymaster/main.yml)](https://github.com/avnu-labs/paymaster/actions)
[![codecov](https://codecov.io/gh/avnu-labs/paymaster/graph/badge.svg)](https://codecov.io/gh/avnu-labs/paymaster)
[![Telegram](https://img.shields.io/badge/Telegram-Join%20Chat-blue?logo=telegram)](https://t.me/avnu_developers)

Sponsor gas fees, accept any token, and control every detail of the gas experience.
SNIP-29 compliant Paymaster for Starknet.

> [!TIP]
> **Just want to integrate? No need to self-host.**
>
> avnu operates a managed Paymaster — no infrastructure to run.
> - **Gasless** (user pays in any token) — no setup required
> - **Gasfree** (you sponsor gas) — API key via [Portal](https://portal.avnu.fi)
>
> [Documentation](https://docs.avnu.fi/docs/paymaster) · [Portal](https://portal.avnu.fi) · Endpoints: `starknet.paymaster.avnu.fi` / `sepolia.paymaster.avnu.fi`

## Features

- **Gasless** — Let users pay gas in any ERC-20 (USDC, ETH, ...)
- **Gasfree** — Sponsor user transactions with flexible logic (API key or webhook)
- **Fast setup** — Deploy a full Paymaster in 2 minutes with the CLI
- **Auto-rebalancing** — Swap supported tokens into STRK and refill relayers automatically
- **Scales effortlessly** — Vertical (more relayers) or horizontal (multi-instance with Redis)
- **Full observability** — OpenTelemetry metrics, logs and traces out of the box
- **SNIP-29 compliant** — Integrates with `starknet.js` and `starknet-react`
- **Extensible by design** — Bring your own price feeds, database, or logic
- **Audited** — Forwarder contract reviewed by Nethermind

## Integrate in your dApp

Works with both `starknet.js` and `starknet-react`:

```ts
// Using avnu's managed instance
const paymasterRpc = new PaymasterRpc({
  nodeUrl: "https://sepolia.paymaster.avnu.fi",
  headers: { "x-paymaster-api-key": "YOUR_API_KEY" },
});

// Or point to your own self-hosted instance
// const paymasterRpc = new PaymasterRpc({ nodeUrl: "http://localhost:12777" });

const account = await WalletAccount.connect(
  STARKNET_PROVIDER,
  STARKNET_WINDOW_OBJECT_WALLET,
  undefined,
  paymasterRpc,
);

const result = await account.executePaymasterTransaction([CALLS], {
  feeMode: { mode: "default", gasToken: "<GAS_TOKEN_ADDRESS>" },
});
```

Full integration guides: [managed instance](https://docs.avnu.fi/docs/paymaster) · [self-hosted](https://docs.out-of-gas.xyz/docs/dapp-integration)

## Installation

### asdf (Recommended)

Install using [asdf](https://asdf-vm.com/) version manager:

```bash
# Add the plugin
asdf plugin add paymaster https://github.com/avnu-labs/paymaster.git

# Install a version
asdf install paymaster latest

# Set it globally (or use .tool-versions file)
asdf set paymaster latest

# Both binaries are now available
paymaster-cli --help
paymaster-service
```

### GitHub Releases

Download pre-built binaries from [GitHub Releases](https://github.com/avnu-labs/paymaster/releases).

Available for: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64).

### Rust Binary

```bash
git clone https://github.com/avnu-labs/paymaster
cd paymaster
cargo build --release --bin paymaster-service
```

### Docker

```bash
docker pull avnulabs/paymaster:latest

# Or build locally:
docker build -t paymaster:latest .

# docker run
docker run --rm -d -p 12777:12777 \
  -e PAYMASTER_PROFILE=/profiles/default.json \
  -v <PROJECT_DIR>/paymaster/profiles/main.json:/profiles/default.json \
  --name paymaster paymaster
```

## Quick Start

Install the CLI and deploy your Paymaster in 2 minutes:

```bash
cargo install --path . --bin paymaster-cli

cargo run --bin paymaster-cli quick-setup \
  --chain-id=sepolia \
  --master-address=0xDEAD \
  --master-pk=0xBEEF \
  --profile=my-profile
```

Then run your Paymaster:

```bash
cargo run --release --bin paymaster-service --profile=path/to/my-profile.json
```

## Documentation

| Resource | Description |
|----------|-------------|
| [docs.avnu.fi/docs/paymaster](https://docs.avnu.fi/docs/paymaster) | Using avnu's managed Paymaster |
| [portal.avnu.fi](https://portal.avnu.fi) | API keys, gas credits, transaction monitoring |
| [docs.out-of-gas.xyz](https://docs.out-of-gas.xyz) | Self-hosting your own instance |
| [Contracts](https://github.com/avnu-labs/paymaster/tree/main/contracts) | Forwarder contracts (audited by Nethermind) |

## Contributing

See [CONTRIBUTING.md](https://github.com/avnu-labs/paymaster/blob/main/CONTRIBUTING.md) to get started.

## License

Licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

You are free to use, modify, and distribute this code. If you run it as a service (SaaS, API, hosted infra), you must also open-source your changes.

> [Full license text](https://www.gnu.org/licenses/agpl-3.0.en.html)

## Questions? Feedback?

Join our dev community: [t.me/avnu_developers](https://t.me/avnu_developers)

Made with care by [avnu](https://x.com/avnu_fi)

## Legal Disclaimer

This software is provided "as is", without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose and noninfringement. In no event shall the authors or copyright holders be liable for any claim, damages or other liability, whether in an action of contract, tort or otherwise, arising from, out of or in connection with the software or the use or other dealings in the software.

Use at your own risk.

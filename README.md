# AVNU Paymaster

💸  Gas abstraction made easy on Starknet  

Open-source. Production-ready. Fully extensible.

[![License: AGPL v3](https://img.shields.io/badge/license-AGPLv3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Docs](https://img.shields.io/badge/docs-available-green)]([https://doc.avnu.fi/avnu-paymaster/](https://docs.out-of-gas.xyz/docs/introduction))
[![Build](https://img.shields.io/github/actions/workflow/status/avnu-labs/paymaster/main.yml)](https://github.com/avnu-labs/paymaster/actions)
[![codecov](https://codecov.io/gh/avnu-labs/paymaster/graph/badge.svg)](https://codecov.io/gh/avnu-labs/paymaster)
[![Telegram](https://img.shields.io/badge/Telegram-Join%20Chat-blue?logo=telegram)](https://t.me/avnu_developers)

Sponsor gas fees, accept any token, and control every detail of the gas experience.
Empower your application with a SNIP‑29 compliant Paymaster.

## ✨ Features

- 💸 **Gasless**: Let users pay in any ERC‑20 (e.g. USDC, DOG, ETH)
- 🆓 **Gasfree**: Sponsor user transactions with flexible logic (API Key or webhook)
- ⚡ **Fast setup**: Deploy a full Paymaster in 2 minutes with the CLI
- 🔁 **Auto-rebalancing**: Swap supported tokens into STRK + refill relayers automatically
- 📈 **Scales effortlessly**: Vertical (more relayers) or horizontal (multi-instance with Redis)
- 🔍 **Full observability**: OpenTelemetry metrics, logs & traces out of the box
- 🔐 **SNIP‑29 compliant**: Integrates with `starknet.js` and `starknet-react`
- 🧩 **Extensible by design**: Bring your own price feeds, database, or logic
- ✅ **Audited & trusted**: Forwarder contract reviewed by Nethermind

## 📦 Installation

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
docker run --rm -d -p 12777:12777  -e PAYMASTER_PROFILE=/profiles/default.json -v <PROJECT_DIR>/paymaster/profiles/main.json:/profiles/default.json --name paymaster paymaster
```

## 🚀 Quick Start

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


## 🧩 Integrate in your dApp

Supports both starknet.js and starknet-react:

```ts
// Starknetjs example
const paymasterRpc = new PaymasterRpc({ 
    nodeUrl: "https://sepolia.paymaster.avnu.fi",
    headers: {'x-paymaster-api-key': 'IF_NEEDED'},
});
// const paymasterRpc = new PaymasterRpc({ default: true });
const account = await WalletAccount.connect(STARKNET_PROVIDER, STARKNET_WINDOW_OBJECT_WALLET, undefined, paymasterRpc);

const result = await account.executePaymasterTransaction(
  [CALLS], 
  { feeMode: { mode: "default", gasToken: "<GAS_TOKEN_ADDRESS>" } }
);

const { transaction_hash } = result;
```

🔗 [Full Integration Guide available here](https://docs.out-of-gas.xyz/docs/dapp-integration)

### Rust Client

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
paymaster-client = { git = "https://github.com/avnu-labs/paymaster" }
```

```rust
use paymaster_client::{PaymasterClient, TransactionBuilder, STRK_TOKEN};

#[tokio::main]
async fn main() -> Result<(), paymaster_client::Error> {
    let client = PaymasterClient::builder("https://sepolia.paymaster.avnu.fi")
        .api_key("YOUR_API_KEY")
        .build()?;

    // Sponsored transaction (gas paid by the paymaster)
    let resp = TransactionBuilder::new(&client)
        .call(your_call())
        .address(your_account_address)
        .sponsored()
        .send(&your_wallet)
        .await?;

    println!("tx hash: {:#x}", resp.transaction_hash);

    // Non-sponsored transaction (gas defaults to STRK)
    let resp = TransactionBuilder::new(&client)
        .call(your_call())
        .address(your_account_address)
        .send(&your_wallet)
        .await?;

    println!("tx hash: {:#x}", resp.transaction_hash);

    // Two-step flow: inspect fees before signing
    let prepared = TransactionBuilder::new(&client)
        .call(your_call())
        .address(your_account_address)
        .gas_token(STRK_TOKEN)
        .build()
        .await?;

    println!("Estimated fee: {:#x}", prepared.fee.estimated_fee_in_strk);
    let resp = prepared.send(&your_wallet).await?;

    println!("tx hash: {:#x}", resp.transaction_hash);

    Ok(())
}
```

## 📖 Documentation

📚 [Full documentation available here](https://docs.out-of-gas.xyz)

## 🧩 Contracts

📝 [Contracts are available here](https://github.com/avnu-labs/paymaster/tree/main/contracts)

## 🛠 Contributing

This guide will help you get started and contribute into the Starknet Paymaster. [Contributing](https://github.com/avnu-labs/avnu-paymaster/blob/main/CONTRIBUTING.md)

## 📄 License

The AVNU Paymaster is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

- 🧠 You are free to use, modify, and distribute this code.
- 🛠️ If you run this project as a service (SaaS, API, hosted infra), you **must also open source your changes**.
- 🤝 This ensures the ecosystem remains open and benefits from improvements.

> Read the full license: [https://www.gnu.org/licenses/agpl-3.0.en.html](https://www.gnu.org/licenses/agpl-3.0.en.html)



## 💬 Questions? Feedback?

Useful links:

- [Tips & Tricks](https://docs.out-of-gas.xyz/docs/good-to-kow)
- [Glossary](https://docs.out-of-gas.xyz/docs/glossary)

Join our dev community: 📲 [https://t.me/avnu_developers](https://t.me/avnu_developers)

Made with ❤️ by [AVNU](https://x.com/avnu_fi)

## ⚠️ Legal Disclaimer

This software is provided "as is", without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose and noninfringement. In no event shall the authors or copyright holders be liable for any claim, damages or other liability, whether in an action of contract, tort or otherwise, arising from, out of or in connection with the software or the use or other dealings in the software.

Use at your own risk.


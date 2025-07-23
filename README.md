# AVNU Paymaster

💸  Gas abstraction made easy on Starknet  

Open-source. Production-ready. Fully extensible.

[![License: AGPL v3](https://img.shields.io/badge/license-AGPLv3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Docs](https://img.shields.io/badge/docs-available-green)]([https://doc.avnu.fi/avnu-paymaster/](https://docs.out-of-gas.xyz/docs/introduction))
[![Build](https://img.shields.io/github/actions/workflow/status/avnu-labs/paymaster/main.yml)](https://github.com/avnu-labs/paymaster/actions)
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

### Rust Binary

```bash
git clone https://github.com/avnu-labs/paymaster
cd paymaster
cargo build --release --bin paymaster-service
```

### Docker

```bash
docker pull avnu/paymaster:latest

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
cargo run --release --bin paymaster-service --profile=my-profile
```


## 🧩 Integrate in your dApp

Supports both starknet.js and starknet-react:

```ts
account.setPaymaster({ url: "https://paymaster.avnu.fi" });

await account.execute([...], {
  feeParams: {
    type: "paymaster",
    gasTokenAddress: "<USDC_TOKEN_ADDRESS>"
  }
});
```
🔗 [Full Integration Guide →](https://docs.out-of-gas.xyz/docs/dapp-integration)

## 📖 Documentation

📚 Full documentation available at: [[https://doc.avnu.fi/avnu-paymaster](https://docs.out-of-gas.xyz/docs/introduction)]

## 🧩 Contracts

 Contracts are available at : [Contacts](https://github.com/avnu-labs/paymaster/tree/main/contracts)

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

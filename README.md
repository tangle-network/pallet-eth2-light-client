<div align="center">
<a href="https://www.webb.tools/">
    
![Webb Logo](./assets/webb_banner_light.png#gh-light-mode-only)
![Webb Logo](./assets/webb_banner_dark.png#gh-dark-mode-only)
  </a>
  </div>
<h1 align="left"> Eth2 Light Client Relayer </h1>

<div align="left" >

[![License Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=flat-square)](https://opensource.org/licenses/Apache-2.0)[![Twitter](https://img.shields.io/twitter/follow/webbprotocol.svg?style=flat-square&label=Twitter&color=1DA1F2)](https://twitter.com/webbprotocol)[![Telegram](https://img.shields.io/badge/Telegram-gray?logo=telegram)](https://t.me/webbprotocol)[![Discord](https://img.shields.io/discord/833784453251596298.svg?style=flat-square&label=Discord&logo=discord)](https://discord.gg/cv8EfJu3Tn)

</div>


### Getting Started
An eth2 -> tangle network relayer for syncing EVM data on Tangle.

### Prerequisites

This repo uses Rust so it is required to have a Rust developer environment set up. First install and configure rustup:

```bash
# Install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Configure
source ~/.cargo/env
```

Configure the Rust toolchain to default to the latest stable version:

```bash
rustup default stable
rustup update
```

Great! Now your Rust environment is ready!

---


### Tangle Setup 
#### 1. Clone Tangle 
```bash
git clone https://github.com/webb-tools/tangle.git
cd tangle
cargo build --release -p tangle-standalone
```
#### 2. Run Tangle Network
```bash
./scripts/run-standalone-local.sh  --clean
```
Note that this will start a clean network state, if you want to continue running on an old state (using old database)
just omit the `--clean` flag.

#### 3. Insert Key
```bash
echo "gown surprise mirror hotel cash alarm raccoon you frog rose midnight enter//webb//0" &> /tmp/empty/secret_key
```
---

### Light Client Setup

#### 1. Install Git LFS

Before getting started, make sure you have [Git LFS installed](../../topics/git/lfs/index.md) in your computer. Open a terminal window and run:

```shell
git-lfs --version
```

If it doesn't recognize this command, you must install it. There are
several [installation methods](https://git-lfs.com/) that you can
choose according to your OS. To install it with Homebrew:

```shell
brew install git-lfs
```

Once installed, **open your local repository in a terminal window** and
install Git LFS in your repository. If you're sure that LFS is already installed,
you can skip this step. If you're unsure, re-installing it does no harm:

```shell
git lfs install
```

#### 2. Configure light client
```bash
# Edit configuration as required
eth2substrate-block-relay-rs/config.toml
```
#### 3. Set Infura API Key
```bash
export ETH1_INFURA_API_KEY="your_infura_key"
``` 

#### 4. Build Light client
```bash
cargo build --release -p node-template
```

#### 5. Run light client
```bash
./target/release/node-template --tmp --chain local --alice  \
  --rpc-cors all --rpc-external --rpc-methods=unsafe \
  --port 30433 \
  --relayer-config-dir=./gadget/config \
  --light-client-init-pallet-config-path=./crates/eth2-pallet-init/config.toml \
  --light-client-relay-config-path=./eth2substrate-block-relay-rs/config.toml
  --rpc-port 9444
```

---

## Contributing

Interested in contributing to the Webb Eth2 light client relayer? Thank you so much for your interest! We are always appreciative for contributions from the open-source community!

If you have a contribution in mind, please check out our [Contribution Guide](./.github/CONTRIBUTING.md) for information on how to do so. We are excited for your first contribution!

## License

Licensed under <a href="LICENSE">GNU General Public License v3.0</a>.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the GNU General Public License v3.0 license, shall be licensed as above, without any additional terms or conditions.

<div align="center">
<a href="https://www.webb.tools/">
    
![Webb Logo](./assets/webb_banner_light.png#gh-light-mode-only)
![Webb Logo](./assets/webb_banner_dark.png#gh-dark-mode-only)
  </a>
  </div>
<h1 align="left"> 🪢 🕸️ Eth2 Light Client Relayer 🕸️ 🪢 </h1>
<p align="left">
    <strong>🚀 An eth2 -> tangle network relayer for syncing EVM data on Tangle 🚀</strong>
</p>

<div align="left" >

[![License Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=flat-square)](https://opensource.org/licenses/Apache-2.0)
[![Twitter](https://img.shields.io/twitter/follow/webbprotocol.svg?style=flat-square&label=Twitter&color=1DA1F2)](https://twitter.com/webbprotocol)
[![Telegram](https://img.shields.io/badge/Telegram-gray?logo=telegram)](https://t.me/webbprotocol)
[![Discord](https://img.shields.io/discord/833784453251596298.svg?style=flat-square&label=Discord&logo=discord)](https://discord.gg/cv8EfJu3Tn)

</div>

## Running the eth2-subtrate-relay on tangle

1. Compile (via `cargo build -p tangle-standalone --release`) then run the tangle network via `./scripts/run-standalone-local.sh` (branch: `develop`)
2. Run `echo "gown surprise mirror hotel cash alarm raccoon you frog rose midnight enter//webb//0" &> /tmp/empty/secret_key`
4. Checkout the relayer repo (working branch: `thomas/eth_light_client`)
5. Edit the file (as needed) in `./services/light-client-relayer/config_relayer.toml`
6. run `cp ./services/light-client-relayer/config_relayer.toml /tmp/empty/`
7. Set the ETH1_INFURA_API_KEY environment variable (i.e., `export ETH1_INFURA_API_KEY="abc123"`)
8. finally, run the relayer: `cargo r --bin webb-light-client-relayer --features cli -- -c /tmp/empty --tmp -vvvv`

### Goal
In this develop target this version of [**rainbow-bridge**](https://github.com/webb-tools/rainbow-bridge/tree/c0d3986941dbf7f730cc3978c8b27e7e292d41fd).

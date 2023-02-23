# pallet-eth2-light-client
A Substrate pallet implementing an ETH2 Beacon Chain light client

## Running the eth2-subtrate-relay on tangle

1. Compile (via `cargo build -p tangle-standalone --release`) then run the tangle network via `./scripts/run-standalone-local.sh` (branch: `thomas/eth-light-client`)
2. Run `echo "gown surprise mirror hotel cash alarm raccoon you frog rose midnight enter//webb//0" &> /tmp/empty/secret
_key`
4. Checkout the relayer repo (working branch: `thomas/eth_light_client`)
5. Edit the file in ./services/light-client-relayer/config_relayer.toml:

Edit the corresponding `signer_account_id`

6. run `cp ./services/light-client-relayer/config_relayer.toml /tmp/empty/`
7. finally, run the relayer: `cargo r --bin webb-light-client-relayer --features cli -- -c /tmp/empty --tmp -vvvv`

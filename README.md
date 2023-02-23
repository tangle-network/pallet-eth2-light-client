# pallet-eth2-light-client
A Substrate pallet implementing an ETH2 Beacon Chain light client

## Running the eth2-subtrate-relay on tangle

1. Compile (via `cargo build -p tangle-standalone --release`) then run the tangle network via `./scripts/run_arana_local.sh` (branch: `thomas/eth-light-client`)
2. The previous command will create private keys. Copy any one of the private keys in `./tmp/standalone1/chains/arana-local/keystore/` to `/tmp/empty`
3. Open the copied private key in `/tmp/empty/` and remove the quotation marks on both ends
4. Checkout the relayer repo (working branch: `thomas/eth_light_client`)
5. Edit the file in ./services/light-client-relayer/config_relayer.toml:

point `path_to_signer_secret_key` to the copied private key in `/tmp/empty`.

Edit the corresponding `signer_account_id`

6. run `cp ./services/light-client-relayer/relayer_config.toml /tmp/empty/`
7. finally, run the relayer: `cargo r --bin webb-light-client-relayer --features cli -- -c /tmp/empty --tmp -vvvv`

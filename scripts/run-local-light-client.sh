#!/bin/bash
set -e
# ensure we kill all child processes when we exit
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

#define default ports
ports=(30433 30405 30408)

#check to see process is not orphaned or already running
for port in ${ports[@]}; do
    if [[ $(lsof -i -P -n | grep LISTEN | grep :$port) ]]; then
      echo "Port $port has a running process. Exiting"
      exit -1
    fi
done

CLEAN=${CLEAN:-false}
# Parse arguments for the script

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -c|--clean)
            CLEAN=true
            shift # past argument
            ;;
        *)    # unknown option
            shift # past argument
            ;;
    esac
done

pushd .

# Check if we should clean the tmp directory
if [ "$CLEAN" = true ]; then
  echo "Cleaning tmp directory"
  rm -rf ./tmp
fi

# The following line ensure we run from the project root
PROJECT_ROOT=$(git rev-parse --show-toplevel)
cd "$PROJECT_ROOT"

echo "*** Start Light client nodes ***"
# Alice
./target/release/node-template --tmp --chain local --alice  \
  --rpc-cors all --rpc-external --rpc-methods=unsafe \
  --port 30433 \
  --relayer-config-dir=./gadget/config \
  --light-client-init-pallet-config-path=./crates/eth2-pallet-init/config_for_tests.toml \
  --light-client-relay-config-path=./eth2substrate-block-relay-rs/config_for_tests.toml
  --rpc-port 9444  &
# Bob
./target/release/node-template --tmp --chain local --bob  \
  --rpc-cors all --rpc-external --rpc-methods=unsafe \
  --port 30405 \
  --relayer-config-dir=./gadget/config \
  --light-client-init-pallet-config-path=./crates/eth2-pallet-init/config_for_tests.toml \
  --light-client-relay-config-path=./eth2substrate-block-relay-rs/config_for_tests.toml
  --rpc-port 9445 --bootnodes /ip4/127.0.0.1/tcp/30433/p2p/12D3KooWCzqQx1oEPJ94uDPXPa2VdHqDD5ftpCFmSd5KPHgxMivK &
# Charlie
./target/release/node-template --tmp --chain local --charlie  \
    --rpc-cors all --rpc-external \
    --rpc-port 9448 \
    --port 30408 \
    --relayer-config-dir=./gadget/config \
    --light-client-init-pallet-config-path=./crates/eth2-pallet-init/config_for_tests.toml \
    --light-client-relay-config-path=./eth2substrate-block-relay-rs/config_for_tests.toml
    --bootnodes /ip4/127.0.0.1/tcp/30433/p2p/12D3KooWCzqQx1oEPJ94uDPXPa2VdHqDD5ftpCFmSd5KPHgxMivK \
    --unsafe-rpc-external --rpc-methods=unsafe
popd

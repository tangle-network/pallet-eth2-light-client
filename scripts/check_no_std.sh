cd crates

cd bls && cargo b --no-default-features && cd ..
cd consensus-types && cargo b --no-default-features && cd ..
cd eth-types && cargo b --no-default-features && cd ..
cd eth2-hashing && cargo b --no-default-features && cd ..
cd merkle-proof && cargo b --no-default-features && cd ..
cd safe-arith && cargo b --no-default-features && cd ..
cd ssz && cargo b --no-default-features && cd ..
cd ssz-derive && cargo b --no-default-features && cd ..
cd serde-utils && cargo b --no-default-features && cd ..
cd tree-hash && cargo b --no-default-features && cd ..
cd tree-hash-derive && cargo b --no-default-features && cd ..
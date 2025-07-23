#!bin/bash

kill $(pgrep starknet-devnet)
rm blockchain.json

starknet-devnet --dump-on exit --dump-path blockchain.json --seed 0 &
STARKNET_PROCESS=$!

sleep 3

# Declare Argent Wallet
RESULT=$(starkli declare --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                contracts/argent/ArgentAccount.json)

ARGENT_CLASS_HASH=$(echo $RESULT | grep 0x | tail)

# Deploy one argent wallet for testing
RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --salt 0x5 \
                --not-unique \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                $ARGENT_CLASS_HASH \
                0x0 \
                0x39d9e6ce352ad4530a0ef5d5a18fd3303c3606a7fa6ac5b620020ad681cc33b \
                0x1)

ARGENT_WALLET=$(echo $RESULT | grep 0x | tail)
echo "ARGENT WALLET $ARGENT_WALLET"

# Deploy one argent wallet for testing
starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                0x49D36570D4E46F48E99674BD3FCC84644DDD6B96F7C741B1562B82F9E004DC7 \
                selector:transfer \
                $ARGENT_WALLET \
                u256:10000000000000000000

starkli declare --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                contracts/paymaster/Forwarder.json

RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --salt 0x0 \
                --not-unique \
                0x054e57545b42b9e06a372026d20238d192bfc5378110670cb0ddb8b295014af9 \
                0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691 \
                0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691)

CONTRACT=$(echo $RESULT | grep 0x | tail)
echo "FORWARDER = ${CONTRACT}"

starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --watch \
                $CONTRACT \
                selector:set_whitelisted_address \
                0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691 \
                0x1

starkli declare --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                contracts/paymaster/Funder.json


RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --salt 0x0 \
                --not-unique \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                0x04be61fe7a73b2cc20359515664fa6c74680d49220a7baeb31e504fe8aaa1ada \
                0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691)

FUNDER=$(echo $RESULT | grep 0x | tail)
echo "FUNDER = ${FUNDER}"

# Deploy Argent Accounts to be used as relayers
RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --salt 0x1 \
                --not-unique \
                $ARGENT_CLASS_HASH \
                0x0 \
                0x39d9e6ce352ad4530a0ef5d5a18fd3303c3606a7fa6ac5b620020ad681cc33b \
                0x1)
CONTRACT=$(echo $RESULT | grep 0x | tail)
echo "RELAYER 1 = ${CONTRACT}"


RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --salt 0x2 \
                --not-unique \
                $ARGENT_CLASS_HASH \
                0x0 \
                0x39d9e6ce352ad4530a0ef5d5a18fd3303c3606a7fa6ac5b620020ad681cc33b \
                0x1)
CONTRACT=$(echo $RESULT | grep 0x | tail)
echo "RELAYER 2 = ${CONTRACT}"


RESULT=$(starkli deploy --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --salt 0x3 \
                --not-unique \
                $ARGENT_CLASS_HASH \
                0x0 \
                0x39d9e6ce352ad4530a0ef5d5a18fd3303c3606a7fa6ac5b620020ad681cc33b \
                0x1)

CONTRACT=$(echo $RESULT | grep 0x | tail)
echo "RELAYER 3 = ${CONTRACT}"

starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --watch \
                $FUNDER \
                selector:set_whitelisted_address \
                0x016f3f34c417aa41782bc641bfcd08764738344034ee760b0d00bea3cdb9b258 \
                0x1

starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --watch \
                $FUNDER \
                selector:set_whitelisted_address \
                0x0365133c36063dabe51611dd8a83ca4a31944ea87bd7ef3da576b754be098dc1 \
                0x1

starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --watch \
                $FUNDER \
                selector:set_whitelisted_address \
                0x055c5d84d644301e4d2375c93868484c94a76bd68a565620bda3473efb4cf9a0 \
                0x1

starkli invoke --rpc http://localhost:5050 \
                --private-key 0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9 \
                --account accounts/account.json \
                --watch \
                0x4718F5A0FC34CC1AF16A1CDEE98FFB20C31F5CD61D6AB07201858F4287C938D \
                selector:transfer \
                $FUNDER \
                u256:100000000000000000000

kill -2 $STARKNET_PROCESS
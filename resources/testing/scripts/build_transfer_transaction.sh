#!/bin/bash

curl -XPOST http://localhost:12777 \
  -H "content-type: application/json" \
  -d '{
    "id": 1,
    "jsonrpc": "2.0",
    "method": "paymaster_buildTransaction",
    "params": [{
        "transaction": {
          "type":"invoke",
          "invoke": {
            "user_address":"0x7395669f79215154f63f45f3e26ea4a53be1665408f94e3d643dd8d03e8065f",
            "calls":[
              {
                "to":"0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
                "selector":"0x83afd3f4caedc6eebf44246fe54e38c95e3179a5ec9ea81740eca5b482d12e",
                "calldata":[
                  "0x40c69b05b7d8c1f0bb6373fccc578865c6019580a4a328da7ff45edfa552060",
                  "0xde0b6b3a7640000",
                  "0x0"
                ]
              }
            ]
          }
        },
        "parameters": {
          "version":"0x1",
          "fee_mode":{
            "mode":"default",
            "gas_token":"0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
          },
          "time_bounds": null
        }
      }]
  }'
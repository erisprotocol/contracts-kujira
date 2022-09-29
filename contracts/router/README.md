# Eris Router <!-- omit in toc -->

The Router Contract contains the logic to facilitate multi-hop swap operations via FIN.

**Only FIN is supported.**

TODO: ADD contract links + example transaction

### Operations Assertion

The contract will check whether the resulting token is swapped into one token.

### Example

Swap Luna => axlUSDC => Kuji

```
{
   "execute_swap_operations":{
      "operations":[
         {
            "swap":{
               "offer_asset_info": "ibc/DA59C009A0B3B95E0549E6BF7B075C8239285989FF457A8EDDBB56F10B2A6986",
               "ask_asset_info": "ibc/295548A78785A1007F232DE286149A6FF512F180AF5657780FC89C009E2C348F"
            }
         },
         {
            "swap":{
               "offer_asset_info":"ibc/295548A78785A1007F232DE286149A6FF512F180AF5657780FC89C009E2C348F",
               "ask_asset_info":"ukuji"
            }
         }
      ],
      "minimum_receive":"1"
   }
}
```

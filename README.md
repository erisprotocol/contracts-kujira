# Eris Amplified Staking

Juno liquid staking derivative. Of the community, by the community, for the community.

A previous version ([v1.0.0-rc0](https://github.com/st4k3h0us3/steak-contracts/releases/tag/v1.0.0-rc0)) of Steak was audited by [SCV Security](https://twitter.com/TerraSCV) ([link](https://github.com/SCV-Security/PublicReports/blob/main/CW/St4k3h0us3/St4k3h0us3%20-%20Steak%20Contracts%20Audit%20Review%20-%20%20v1.0.pdf)).

## Contracts

| Contract                               | Description                                              |
| -------------------------------------- | -------------------------------------------------------- |
| [`erist-staking-hub`](./contracts/hub) | Manages minting/burning of ampJUNO token and bonded Juno |

## Deployment

### Mainnet

| Contract                  | Address                                                                                                                                                                           |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Eris Staking Hub          | [`terra10788fkzah89xrdm27zkj5yvhj9x3494lxawzm5qq3vvxcqz2yzaqyd3enk`](https://finder.terra.money/mainnet/address/terra10788fkzah89xrdm27zkj5yvhj9x3494lxawzm5qq3vvxcqz2yzaqyd3enk) |
| Eris Liquid Staking Token | [`terra1ecgazyd0waaj3g7l9cmy5gulhxkps2gmxu9ghducvuypjq68mq2s5lvsct`](https://finder.terra.money/mainnet/address/terra1ecgazyd0waaj3g7l9cmy5gulhxkps2gmxu9ghducvuypjq68mq2s5lvsct) |
| ampJUNO-JUNO Pair         | [`terra1ccxwgew8aup6fysd7eafjzjz6hw89n40h273sgu3pl4lxrajnk5st2hvfh`](https://finder.terra.money/mainnet/address/terra1ccxwgew8aup6fysd7eafjzjz6hw89n40h273sgu3pl4lxrajnk5st2hvfh) |
| ampJUNO-JUNO LP Token     | [`terra1eh2aulwsyc9m45ggeznav402xcck4ll0yn0xgtlxyf4zkwch7juqsxvfzr`](https://finder.terra.money/mainnet/address/terra1eh2aulwsyc9m45ggeznav402xcck4ll0yn0xgtlxyf4zkwch7juqsxvfzr) |

### Testnet

| Contract                  | Address                                                                                                                                                                           |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Eris Staking Hub          | [`terra1cgurv08h780ygh3a4l2tjtxndksywskxp4mypkazuuazqas5m8kqleeupz`](https://finder.terra.money/testnet/address/terra1cgurv08h780ygh3a4l2tjtxndksywskxp4mypkazuuazqas5m8kqleeupz) |
| Eris Liquid Staking Token | [`terra1ucfhdxddqs37lkfpv5lze7lr73lf90jy7zcjppredcxc3v2pgakqppaflr`](https://finder.terra.money/testnet/address/terra1ucfhdxddqs37lkfpv5lze7lr73lf90jy7zcjppredcxc3v2pgakqppaflr) |
| ampJUNO-JUNO Pair         | [`terra14lr9zdfn0d5gxjwafh3mg5nrrculj4dndunynve452zws2lzyd3smx46ta`](https://finder.terra.money/testnet/address/terra14lr9zdfn0d5gxjwafh3mg5nrrculj4dndunynve452zws2lzyd3smx46ta) |
| ampJUNO-JUNO LP Token     | [`terra1evucal9yqpa9fcgvfdengy7vldrgsa623900f6s6605dwnf4qpnqke06cc`](https://finder.terra.money/testnet/address/terra1evucal9yqpa9fcgvfdengy7vldrgsa623900f6s6605dwnf4qpnqke06cc) |

## Building

For interacting with the smart contract clone <https://github.com/erisprotocol/liquid-staking-scripts> into the same parent folder.

## Changes

- Renaming
- Update to CosmWasm 1.0.0
- added a reward fee for running the protocol
- added schema generation
- added a more detailed unbonding query
- Fixed an issue in reconciliation when the expected Juno was correct the unbinding queue items were not marked reconciled
- move scripts to another repository, so that the repo of the smart contracts will not be touched as much <https://github.com/erisprotocol/liquid-staking-scripts>

## Changelog

### Hub Version 1.1.0

- Support new execute operation "donate" to add JUNO without minting ampJUNO. Will be used to increase the exchange_rate
- Extended "state" query to include the full tvl in ujuno.
- Added migration version handling

## License

Contents of this repository are open source under [GNU General Public License v3](./LICENSE) or later.

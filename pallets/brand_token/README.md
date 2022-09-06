Struct
Token {
    symbol: String
    amount: number (amount of staked native token to mint brand token equally)
    staked:
    default-lifetime
}

Storage
BrandTokenById: brand id - Token struct
UTXO: (brand id, account id) - [(amount, expire)]

Extrinsics
+ create()
+ mint()
+ burn()
+ transfer()
+ earn(lifetime: months, amount)

Questions
+ Vi la token expire nen la se khong exchange brand token (order book/pool/offer) 
hoac sau khi exchange thi dc reset expire theo quy dinh cua brand token (later)
+ Con exchange nft thi mua/ban qua native token

- feedback, 
- UC: voting on club decisions, rewards, merchandise designs and unique experiences, serving as a ticket into a secure, exclusive inner circle of fans . (depend on brand)
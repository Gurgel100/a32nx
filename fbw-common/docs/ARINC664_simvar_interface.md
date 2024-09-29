# ARINC664 simvar interface
In ARINC664 a single dataset consists of multiple fields of different types.
To transfer datasets from/to the AFDX network each field of a dataset gets its own simvar.

## Naming
The simvar should follow the following naming scheme:
`${prefix}_ACDN_${NAME OF AFDX VALUE}_${NAME OF FIELD}_${i}`

## Value Format
|sign bit(1bit)|exponent(11bits)|mantissa(52bits)|
|--------------|----------------|----------------|
|unused        |used for status and type|data    |

## Exponent
|0-1|2-5|6-9|10|
|---|---|---|--|
|status|type|unused|always `1` (to not have a denormal value)|

## Types
|Type|Value|Remarks|
|----|-----|-------|
|Signed Integer 32 bit|0b000||
|Signed Integer 64 bit|0b001|Only 52 bits usable|
|Float 32 bit|0b010||
|Float 64 bit|0b011|not possible via simvars|
|Strings (ASCII)|0b100|only 6 characters, could be increased to 7 by using 7-bit encoding|
|Opaque Data (Byte arrays)|0b101|6 bytes|

## Status
|Meaning|Value|
|-------|-----|
|no data (ND)|0b00|
|no computed data (NCD)|0b01|
|functional test (FT)|0b10|
|normal operation (NO)|0b11|

Apple's public root certificates, from https://www.apple.com/certificateauthority/,
which the App Store's signed transactions and notifications chain to; see
`src/accounts/app_store.rs`. Public, not secrets.

| File | SHA-256 fingerprint |
|---|---|
| AppleRootCA-G3.cer | 63:34:3A:BF:B8:9A:6A:03:EB:B5:7E:9B:3F:5F:A7:BE:7C:4F:5C:75:6F:30:17:B3:A8:C4:88:C3:65:3E:91:79 |
| AppleRootCA-G2.cer | C2:B9:B0:42:DD:57:83:0E:7D:11:7D:AC:55:AC:8A:E1:94:07:D3:8E:41:D8:8F:32:15:BC:3A:89:04:44:A0:50 |

Checked with `openssl x509 -inform der -in <file> -noout -fingerprint -sha256`.

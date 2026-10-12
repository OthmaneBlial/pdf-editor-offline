# IBM Plex Sans UI fonts

The unmodified complete regular and medium TrueType fonts are embedded for offline native UI typography. IBM Plex Sans Regular leads the proportional family; Medium supplies headings and buttons. The Noto symbol fallback remains last. No proprietary platform font is copied or distributed.

Official upstream: [IBM / Plex](https://github.com/IBM/plex/tree/c5f949677f6f163e8dfe98ca2c326bd48b42fa1b/packages/plex-sans/fonts/complete/ttf), pinned commit `c5f949677f6f163e8dfe98ca2c326bd48b42fa1b` (2024-06-12).

| Payload | Bytes | SHA-256 |
| --- | ---: | --- |
| `IBMPlexSans-Regular.ttf` | 200,500 | `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5` |
| `IBMPlexSans-Medium.ttf` | 202,460 | `331c8639d7598b2cde62a911a71db195e30cb655cd6bdf2e324a7e984955f907` |
| `OFL.txt` (upstream `license.txt`) | 4,360 | `91c25c350d3cac39da2736d74f7ba37ef648f5237a4e330a240615bc8d8c4360` |

Download base: `https://raw.githubusercontent.com/IBM/plex/c5f949677f6f163e8dfe98ca2c326bd48b42fa1b/packages/plex-sans/fonts/complete/ttf/`.

Copyright 2017 IBM Corp., Reserved Font Name "Plex". SIL Open Font License 1.1; the exact upstream notice is retained in [OFL.txt](OFL.txt). Native distributions must bundle this notice alongside the Noto font notice.

```sh
shasum -a 256 apps/desktop/assets/fonts/ibm-plex/*.ttf apps/desktop/assets/fonts/ibm-plex/OFL.txt
```

# Row icons

## Amazon Q icons

The 42 PNGs not listed in `simple-icons.txt` are the autocomplete icons of
[aws/amazon-q-developer-cli-autocomplete](https://github.com/aws/amazon-q-developer-cli-autocomplete), used under its
Apache-2.0 licence (<https://www.apache.org/licenses/LICENSE-2.0>).

Changes made to them: `file` and `folder` are resized to 32×32 with `sips`. `template.rgba` is `template.png` decoded
to 32×32 RGBA8, so the tint can be applied in code.

## Brand icons from simple-icons

The PNGs listed in `simple-icons.txt` are drawn by `just brand-icons` (`scripts/brand-icons.sh`, `tools/brand-icons`)
from [simple-icons](https://simple-icons.org) 16.38.1, whose icons are CC0-1.0
(<https://creativecommons.org/publicdomain/zero/1.0/>). Each one is `template.png` tinted with the brand's colour, as
tabsmith tints badges, with the simple-icons glyph centred on it in white, or in black on a light colour.

To add a brand, add its simple-icons slug and colour to `simple-icons.txt` and run `just brand-icons`. To drop one,
delete its line and its PNG. The tool refuses entries whose licence does not allow this use (share-alike, copyleft,
non-commercial, no-derivatives or custom terms). These entries carry an attribution licence of their own:

- `apachemaven`, Apache-2.0: <https://apache.org/logos>
- `deno`, MIT: <https://github.com/denoland/docs/blob/5dee713844c7447f80acd4093caa9d350d80bf36/static/img/logo.svg>
- `exercism`, CC-BY-3.0: <https://github.com/exercism/website-icons/blob/2ad12baa465acfaa74efc5da27a6a12f8b05e3d0/exercism/logo-icon.svg>
- `gnubash`, MIT: <https://github.com/odb/official-bash-logo/tree/61eff022f2dad3c7468f5deb4f06652d15f2c143>
- `openjdk`, BSD-3-Clause: <https://hg.openjdk.java.net/duke/duke/file/ca00f100dafc/vector/Agent.svg>
- `sanity`, MIT: <https://github.com/sanity-io/logos/blob/e298a1ac2757c83910fe08b496a115bc9aef06bd/src/sanityMonogram.tsx>
- `typeorm`, MIT: <https://github.com/typeorm/typeorm/blob/e7649d2746f907ff36b1efb600402dedd5f5a499/resources/logo_big.png>
- `v`, MIT: <https://github.com/vlang/v-logo/blob/eec050c901ed3afefce8cbe56092d55ed6770706/dist/v-logo.svg>

## Trademarks

The brand logos (`git`, `docker`, `aws`, `python`, `pnpm`, …) are trademarks of their owners. They are included only
to identify each tool on its row, not to suggest any endorsement.

The three embedded version 4 matrix/TRC profiles come from Compact ICC Profiles commit bdd84663061bc4ae95ca70decff54f581e27f702 and are dedicated to the public domain under CC0. The encoder writes the matching cICP code points for each file. [Upstream profiles](https://github.com/saucecontrol/Compact-ICC-Profiles/tree/bdd84663061bc4ae95ca70decff54f581e27f702/profiles) · [CC0 dedication](LICENSE)

The two PRINT_CMYK device links map sRGB to the press characterizations with Medium GCR. ArgyllCMS 3.5.0 inverted the original ECI A2B tables using its perceptual mapping and a target of half the attainable black locus across the gamut. The destination TAC is 300% for coated and 280% for uncoated. The project's links use 16-bit LUT entries. Select the named ECI destination profile when exporting. The application validates the original profile's SHA-256 and embeds it in both press files. [ECI Offset 2009 profiles](https://eci.org/lib/exe/eci_offset_2009.zip) · [ArgyllCMS collink](https://www.argyllcms.com/doc/collink.html)

Reproduce the links from the extracted ECI files and the v2 sRGB reference shipped with ArgyllCMS.

```sh
collink -v -qh -G -ip -Kh -l300 -L95 -D 'TSS sRGB to coated 50 percent K locus' "$ARGYLL_DIR/ref/sRGB.icm" ISOcoated_v2_300_eci.icc coated-gcr.icc
collink -v -qh -G -ip -Kh -l280 -L98 -D 'TSS sRGB to uncoated 50 percent K locus' "$ARGYLL_DIR/ref/sRGB.icm" PSO_Uncoated_ISO12647_eci.icc uncoated-gcr.icc
```

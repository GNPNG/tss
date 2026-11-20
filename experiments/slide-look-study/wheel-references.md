# Mathmos liquid wheel reference colours

The seven 840 × 840 projection photographs supplied with this project define the reference set. The table gives approximate display RGB colours observed in the projected field. Each row describes a single photograph. The named wheel colours and the colours seen on the screen can differ substantially.

| Wheel | Source photograph | Clear carrier or bright opening | Coloured pools, crossings and rims |
|---|---|---|---|
| Blue/Green | `codex-clipboard-b530d099-810a-4107-b6b8-c07b370bf26f.png` | Cool pale `#BCDCE1` | Bright cyan `#16B6EA`, leaf greens `#51AA70` and `#359350`, deeper emerald `#137547` |
| Blue/Red | `codex-clipboard-e5a441c6-14c4-4027-8de1-337dfe99eaff.png` | Pale mauve `#D8C9DB` | Red `#D51F29`, blue `#0962AF` and `#253E8E`, extensive near-black/brown crossings `#170F0F` |
| Blue/Yellow | `codex-clipboard-600a3498-9a38-474e-90c1-48bb77c54451.png` | Pale yellow-green `#E2E7C5` | Lemon `#F3E963`, olive-green `#91B256`, emerald `#077E4F`. Narrow blue to teal bands reach `#0193CB` |
| Violet/Blue | `codex-clipboard-c881f248-7a4b-4966-993d-b17d24759e84.png` | Cool pale `#D4CCD9` | Pink-violet `#AF6CAB`, cyan `#09A7CF`, pale cyan `#74CDE8`, blue crossings `#126EAB` and `#294485` |
| Violet/Green | `codex-clipboard-49ee8ae4-1462-43a0-9132-49af59513384.png` | Warm cream `#F7E9D5` | Mint `#AED2BA`, rose `#EAACC6`, and muted grey-lavender `#72749A`. The darker purple regions approach `#59476F` |
| Violet/Red | `codex-clipboard-1f674674-361f-4288-9b55-0b70c987151e.png` | Blush-white `#FCEAEE` | Fuchsia `#D92B8C`, brighter pink `#E75D98`, broad scarlet `#DF1D3E`. Small peripheral regions fall to plum `#742D43` |
| Yellow/Red | `codex-clipboard-80d8900c-8943-455f-b44e-24f9a8f29b53.png` | Cream `#E6DFCE` | Yellow `#F8D13D`, amber `#E9AB26`, dominant red `#C82338`. Amber rims and falloff approach `#BD7933` |

### Sampling

The input is the unresized RGB raster of each supplied PNG, interpreted as 8-bit display RGB. Coordinates start at the upper-left pixel. For the palette clusters, sample every fourth pixel along both axes, starting at x = 20 and y = 70, stopping before x = 676 and y = 710. Keep pixels satisfying `((x−345)/310)² + ((y−402)/291)² ≤ 0.90`. Exclude `x > 443 and y > 360` to avoid the overlaid wheel, mount and foreground hardware. Cluster the samples in RGB code-value space with eight centroids. Seed with the middle row-major sample, then successively choose the point farthest from the nearest existing centroid. Perform 18 nearest-centroid assignment and arithmetic-mean updates. Round channel means to the nearest integer. Diffuse projector falloff and variation across a pool both contribute to these means.

The two wheels below also have spot measurements. Each code is a mean over a 7 × 7 square centred on the listed coordinate. These resolve narrow or easily confused features omitted by a dominant-colour palette.

| Wheel | Feature in photograph | Position (x, y) | Sample |
|---|---|---|---|
| Blue/Yellow | Pale upper opening | 370, 250 | `#E8EBD6` |
| Blue/Yellow | Lemon band | 400, 376 | `#E2E15E` |
| Blue/Yellow | Bright lemon near contact | 234, 385 | `#FCF16E` |
| Blue/Yellow | Left green body | 113, 406 | `#47A04D` |
| Blue/Yellow | Deep emerald in the upper-left pool | 175, 272 | `#007C54` |
| Blue/Yellow | Blue band along the curved boundary | 273, 282 | `#0193CB` |
| Violet/Red | Clear central opening | 487, 313 | `#FDEFF2` |
| Violet/Red | Central pink field | 316, 331 | `#EA5FA3` |
| Violet/Red | Upper-right fuchsia pool | 541, 222 | `#D52B92` |
| Violet/Red | Scarlet over the central tongue | 323, 224 | `#E01D3D` |
| Violet/Red | Scarlet adjacent to a crossing | 345, 180 | `#DF1D3E` |
| Violet/Red | Peripheral plum at left | 53, 353 | `#7F2C6A` |

These photographs show the complete optical stack. Assignment of an observed colour to an isolated dye, its thickness, or its overlap with another slide is an appearance-model inference. Lamp spectrum, exposure, colour processing, slide order and local path length are unknown. Use the measured codes for comparison with projected output. Keep the layer interfaces independently simulated.

# Contributing a Legends pack

A Legends pack describes one or more projection setups that a performer can audition on the tracks already in a TSS project. GNPNG is a not-for-profit promoting science and technology for human good. Artists, operators and archivists can contribute as little as a correction to a lamp model or as much as a complete rig with notes on how to play it.

## Try a pack

Put a `.tsslegends.json` file in the `legend-packs/` folder beside the Mac `.app` or Windows `.exe`. Open **Legends** and choose **Install pack** beside its title in **Available packs**. Use **Install pack…** to browse for a file elsewhere. The carousel contains only packs you install and enable. Try the original **Two-beam studio** example or either of GNPNG's independent historical studies. Both studies identify their published sources and explain which settings were chosen for the simulation. [Studio example](../examples/legend-packs/two-beam-studio.tsslegends.json) · [Joshua Light Show research pack](../legend-packs/research/joshua-light-show.tsslegends.json) · [Bill Ham research pack](../legend-packs/research/bill-ham.tsslegends.json)

Use **Previous rig** and **Next rig** to browse. With the carousel focused, the left and right arrows step through rigs and Home or End jumps to the first or last. **Audition** previews the rig on your current tracks. **Stop audition** restores their previous setup. **Apply rig** copies the settings into the project, where they can be edited or undone.

A freshly installed pack is enabled. In **Installed packs**, uncheck **Enabled** to hide its rigs and end an active audition of one of them. Check it again to bring them back. **Remove pack** deletes the installed copy. Saved projects retain the settings you applied. For a performance in a physical room, measure throw and align the audience image to the screen. [Projector positioning and registration plate](projector-positioning.md)

## Describe a rig

Copy the example, give the pack its own stable `id` and title, and start with `version` set to `1.0.0`. Use lowercase letters, digits and separating hyphens for IDs. Each entry in `rigs` has its own ID within the pack. Add entries to that array for different venues, tours or documented configurations. To publish a correction, keep the pack ID and increment its version. Installing that revision requires an explicit **Replace pack** and preserves the owner's enabled setting. A pack holds up to 32 rigs in a 256 KiB UTF-8 JSON file. [Pack format and validation](../WebAssets/legend-packs.js)

| Field | What belongs here |
| --- | --- |
| `artist`, `venue`, `credits` | Identify the people, collective and setting as they wish to be named. Include individual roles such as liquid operator, projectionist, equipment builder, photographer or researcher. |
| `record`, `sources` | Describe the documented apparatus or technique. Pair each source label with its HTTPS address. A dated photograph, operator interview, cue sheet or equipment manual can establish different parts of the same rig. |
| `setup` | Explain what the performer can do with this simulation. Identify estimated settings and significant differences, such as a flat sealed slide standing in for nested clock crystals. |
| `rig` | Select both projectors by equipment ID. Temperatures are kelvin, `levelB` is relative light output and `offsetX` is a fraction of the screen width. `focusA` and `focusB` set the respective focal planes on a 0–1 scale. [Equipment definitions](../WebAssets/equipment.js) |
| `review` | State who contributed or reviewed what, and for which version. The gallery displays the pack author's declaration. GNPNG keeps any permission to distribute separately. |

Use `original` for a new rig of your own design and `research` for a historical interpretation under study. Use `artist-contributed` for an identified contribution, or `artist-reviewed` for an identified review, with the contributor's agreement on the credit. An operator might confirm the hand movement while leaving the original lamp model unknown. Say exactly that in `review.note`. Keep previous versions identifiable when revising a pack.

## Contribute to a historical study

GNPNG develops and shares independent historical simulations under the `research` label using its own text and rig settings. Artist review is an invitation. One operator might be able to confirm who controlled each beam. Another might remember the dish geometry, where the beams met or when a particular cue changed. A short capture of the rig in use lets an operator comment on timing and gesture without installing the editor.

When someone offers material for inclusion, agree on the pack ID and version, the item, the exact credit and where GNPNG may distribute it. An invitation to inspect a personal notebook, for example, may lead to a correction to a projector model. Publishing a scan of the notebook or quoting its author is a separate decision. For a photograph, clip, drawing or attributed quotation, record the agreed scope with the contributor and relevant rights holder. Version 1 of the pack format carries text, source links and rig settings. Links can direct readers to an artist's own archive.

Keep correspondence and unpublished material with the GNPNG project contact. Confirm the artist's preferred wording before describing a pack or revision as a collaboration or endorsing it in the artist's name. Send corrections, credit changes and requests to withdraw an entry from future collections to that same contact. A distributor can omit the pack from its next collection. Anyone who has installed a copy retains control of it in **Installed packs**.

## Record the room for a future 3D reconstruction

Version 1 renders the projected image on a shared screen. A future venue model would need the original equipment housings and lens locations, the aim and position of each projector, any mirrors in the light path, the size and type of screen, and the observer's position. An original 3D model of a projector shell can identify the machine in a rig picker. The lens, its optical axis and the path to the screen determine where the beam actually lands. Give dimensions with units and identify the photograph, drawing, measurement or recollection supporting each one. [Fillmore East room observations and open dimensions](history/joshua-light-show.md#room-and-screen)

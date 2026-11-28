# Joshua Light Show · Liquid Loops and the Fillmore East

MoMA dates *Liquid Loops* to 1967. Its collection object is a silent color transfer from 35 mm film, running 7 minutes 3 seconds. MoMA describes an improvised oil, water and glycerin projection and explains that the group also combined filmed liquid performances with live light and carousel slides in concert. The Fillmore East residency began in March 1968. [MoMA collection record and film still, object 255.2016](https://www.moma.org/collection/works/200467) · [Joshua Light Show’s account of the residency](https://www.joshualightshow.com/about-classic/joshua-light-show-1967-68)

## Watch and compare

- For liquid motion, watch the transfer credited to light artist Cecily Hoyt. The upload and International Film Festival Rotterdam give 1969, while MoMA gives 1967. The sources leave the relationship among those dates and editions open. [Watch *Liquid Loops*](https://www.youtube.com/watch?v=TW733Ut5zE0) · [IFFR screening record](https://iffr.com/en/iffr/2006/films/joshua-light-show-liquid-loops)
- For the concert image, watch the Fillmore East footage presented by Historic Films, then compare Elliott Landy’s photographs. These show the scale and placement of the projections behind the performers. [Fillmore East archival footage](https://www.youtube.com/watch?v=hBf7CPangGA) · [Landy’s *Light Shows* photographs and first-person account](https://www.elliottlandy.com/portfolio-items/light-shows/)
- For the rig and the hand work, the group’s classic history identifies the equipment and describes what the operators did. The group also links footage of later studio practice on *Liquid Loops II* from its own site. [Classic rig and performers](https://www.joshualightshow.com/about-classic/joshua-light-show-1967-68) · [*Liquid Loops II* behind the scenes, 2011](https://www.joshualightshow.com/videos-liquid-loops-ii/liquid-loops-ii-behind-the-scenes-2012)

## Apparatus and operators

Joshua White recalls starting with readily available overheads and Kodak Carousel slide projectors. Once the group had a residency, it modified the machines and built more powerful projectors. Rear projection let the team test new optical arrangements behind the screen. White describes his role as mixing the images from the various operators. [White’s first-person account, paragraphs beginning “I was working in light” and “We started out”](https://www.pooterland.com/lightshows_joshua.html)

The inventory published by the group includes three film projectors, eight carousel slide projectors in two banks, and three overhead projectors. Greg Zinman’s account identifies Cecily Hoyt and Bill Schwarzbach as the liquid specialists. With colored liquid between nested curved clock crystals, pressing the smaller glass against the larger displaced liquid toward the rim. The operators could build up pressure, hold a shape and release it in time with the players. The rack could also project hand-etched loops and photographic slides onto the same screen. [Greg Zinman, group archive](https://www.joshualightshow.com/about-classic/joshua-light-show-1967-68)

For a model-specific comparison, the Science Museum Group catalogues a 1961 Kodak Carousel 550R with a five-inch f/3.5 Ektanar lens, rack-and-pinion focus and a high/low lamp switch. White identifies the Carousel family. The exact model, lens, lamp temperature, beam intensity, dye concentrations, vessel curvature and operator pressure for a particular shot of *Liquid Loops* are unverified here. [550R museum specimen and photographs, object 1990-5036/7646](https://collection.sciencemuseumgroup.org.uk/objects/co205500/kodak-carousel-projector-model-550r) · [White’s equipment history](https://www.pooterland.com/lightshows_joshua.html)

## Play Fillmore Ghost

*Fillmore Ghost* is an editable homage to that separation of duties. One operator shapes colored liquid. A second light adds a moving figure. Both reach the same screen. TSS uses three flat, sealed liquid cells for this example. Each conserves its own volume and has an independent seed and focal depth. Pressure deforms the selected liquid within its slide. The nested, curved clock-crystal boundary described above would require a different physical model.

| Projector | Source | Starting controls |
| --- | --- | --- |
| A | Carmine, cyan and marigold live oil cells, ordered from rear to front | Depths 12%, 50%, 86%. Focus 50%. Lamp 3200 K, intensity 72%. |
| B | An original monochrome moth drawn in motion on black | Six-second loop at 24 fps. Lamp 6500 K, intensity 8%. Black contributes zero light to the combined screen. |

The colors, temperatures, intensity ratio, moth design and its flight path are artistic choices for this example. The only video asset is the motif on projector B. Projector A calculates all oil motion in the live simulator. Historical media are linked at their sources.

1. Keep the two example files side by side in one folder. Start the native TSS editor with its Basilisk solver and FFmpeg media tools available. Choose **Open project** and select the TSS file. [Fillmore Ghost project](../../examples/fillmore-ghost.tss) · [Companion moth film](../../examples/fillmore-moth.mp4)
2. On first open, the editor finds the adjacent film by name, verifies its SHA-256 and prepares its frames. If the file is elsewhere, select **Moth film**, open **Artwork settings**, choose **Relink source**, then select the original MP4. The project runs on the native performance clock when no song is loaded. [Visual media and relinking](../visual-media.md)
3. Switch projector B off and back on to see its contribution. Set A’s intensity to zero to examine the moth alone. Return A to 72%, select **Cyan**, then drag on the projection with the tug gesture. Move **Projector focus** between the three depths while the moth continues along its own path.

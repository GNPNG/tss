# Cake23 shader extraction study

From this directory, retrieve the study input at its pinned upstream revision. The wrapper reads the original Razor templates under `vendor/Cake23`, extracts the graphics scripts and writes the study output under `output`.

```sh
git clone https://github.com/Flexi23/Cake23.git vendor/Cake23
git -C vendor/Cake23 checkout --detach af7b51e83709bcdd9cca33fc2aa2cbff6beb5460
node build-study.mjs
```

The upstream checkout carries its own license. The extraction harness and the pinned lava-lamp template can be inspected independently. [Harness](build-study.mjs) · [Pinned upstream template](https://github.com/Flexi23/Cake23/blob/af7b51e83709bcdd9cca33fc2aa2cbff6beb5460/Cake23/WebTemplates/lavalamp.cshtml)

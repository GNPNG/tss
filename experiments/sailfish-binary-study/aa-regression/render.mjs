import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [, , source, destination, expected = "after"] = process.argv;
if (!source || !destination || !["before", "after"].includes(expected)) {
  throw Error("Usage: node render.mjs SAILFISH_CHECKOUT OUTPUT_DIRECTORY [before|after]");
}
const template = (name) => readFileSync(join(source, "sailfish/templates", name + ".mako"), "utf8");
const definitions = (file, names) => [...template(file).matchAll(/<%def name="(\w+)[^"]*"[^>]*>[\s\S]*?<\/%def>/g)]
  .filter(([, name]) => !names || names.includes(name)).map(([body]) => body).join("\n");
const helpers = [
  definitions("kernel_common", ["get_global_idx", "local_indices", "local_indices_split",
    "indirect_index", "shared_mem_propagation_vars", "load_node_type", "get_dist"]),
  definitions("opencl_compat", ["barrier"]),
  definitions("mako_utils", ["cond"]),
  definitions("propagation"),
].join("\n");
const collisionHeaders = ["binary_shan_chen", "lb_binary_fluid"].flatMap((name, index) => {
  const matches = template("models/" + name).match(/<% needs_barriers = [\s\S]*?if \(!isPropagationOnly\(type\)[^\n]*\{/g) ?? [];
  assert.equal(matches.length, index === 0 ? 1 : 2, name);
  return matches;
});
const header = [
  "#define BLOCK_SIZE 8", "#define DIST_SIZE 336", "#define getGlobalIdx(x,y) ((y)*48+(x))",
  "#define isPropagationOnly(type) ((type)==2)", "#define isExcludedNode(type) ((type)==1)",
  "#define decodeNodeType(ncode) (ncode)", "#define decodeNodeOrientation(ncode) 0",
  "typedef struct { float ${', '.join(grid.idx_name)}; } Dist;",
].join("\n");
const kernels = collisionHeaders.map((body, i) => [
  `__kernel void Probe${i}(__global const int *map, __global float *out, unsigned iteration_number) {`,
  body,
  "%for i,name in enumerate(grid.idx_name):",
  "  d0.${name} = (float)(1000 * ${i+1} + gi);",
  "%endfor",
  "}", "${propagate('out','d0')}", "}",
].join("\n")).join("\n");

mkdirSync(destination, { recursive: true });
for (const pattern of ["AA", "AB"]) {
  for (const onRead of [false, true]) {
    const mode = pattern + "-" + (onRead ? "read" : "write");
    // Render real D2Q9 propagation with a fixed marker in place of collision.
    const settings = `<%!
from sailfish import sym
grid = sym.D2Q9
dim = 2
node_addressing = 'direct'
access_pattern = '${pattern}'
propagate_on_read = ${onRead ? "True" : "False"}
backend = 'opencl'
shared_var = '__local'
barrier_needs_all_threads = propagation_enabled = propagation_sentinels = True
supports_shuffle = propagate_with_shuffle = periodic_x = False
periodicity = (False,False,False)
boundary_size = 0
block_size = 8
envelope_size = 1
arr_nx = grid_nx = 48
lat_nx = 41
bnd_limits = (41,7,1)
dist_size = 336
loc_names = ('gx','gy','gz')
%>`;
    const input = join(destination, mode + ".mako");
    const output = join(destination, mode + ".cl");
    writeFileSync(input, [settings, helpers, header, kernels, ""].join("\n"));
    execFileSync(process.env.MAKO_RENDER || "mako-render", ["--output-file", output, input], {
      stdio: "inherit", env: { ...process.env, PYTHONPATH: resolve(source) },
    });
    const check = spawnSync(process.env.CLANG || "clang", ["-x", "cl", "-cl-std=CL1.2", "-fsyntax-only", output], { encoding: "utf8" });
    if (check.error) throw check.error;
    if (expected === "before" && pattern === "AA") {
      assert.notEqual(check.status, 0, mode);
      assert.match(check.stderr, /use of undeclared identifier 'alive'/, mode);
      console.log(mode, "reproduced undeclared alive");
    } else {
      assert.equal(check.status, 0, check.stderr);
      console.log(mode, "compiled 3 kernels");
    }
  }
}

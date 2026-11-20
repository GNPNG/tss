"""Blender 4.5 scene recipe for measuring light through two state-048 slides.

Run with the Blender background command in README.md. Blender imports the
precomputed PLYs and Cycles performs all light transport on its CPU backend.
"""
import argparse
from pathlib import Path
import sys

import bpy


class ReceiverStudy:
    """Build one scene from fresh factory settings and write one linear receiver image."""

    def __init__(self, options: argparse.Namespace) -> None:
        """Validate new output paths and scene settings before changing Blender state."""
        self.options = options
        self.scale = options.units_per_metre
        if options.output.exists() or (options.save_scene and options.save_scene.exists()):
            raise FileExistsError("Choose fresh output paths")
        if options.width < 16 or options.width % 16 or min(options.samples, options.threads, self.scale) <= 0:
            raise ValueError("Choose a positive multiple of 16 for width and positive sampling, threads and scale")
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.scene = bpy.context.scene

    @classmethod
    def from_cli(cls) -> "ReceiverStudy":
        """Read Blender's arguments after the double hyphen and return the configured study."""
        parser = argparse.ArgumentParser()
        parser.add_argument("geometry", type=Path)
        parser.add_argument("output", type=Path)
        parser.add_argument("--shape", choices=("curved", "flat"), required=True)
        parser.add_argument("--width", type=int, default=640)
        parser.add_argument("--samples", type=int, default=96)
        parser.add_argument("--seed", type=int, default=48)
        parser.add_argument("--threads", type=int, default=2)
        parser.add_argument("--save-scene", type=Path)
        parser.add_argument("--no-shadow-caustics", action="store_true")
        parser.add_argument("--units-per-metre", type=float, default=30000)
        parser.add_argument("--empty-beam", action="store_true")
        return cls(parser.parse_args(sys.argv[sys.argv.index("--") + 1:]))

    def configure(self) -> None:
        """Set Cycles CPU sampling, physical unit scale and float output on the empty scene."""
        options = self.options
        scene = self.scene
        scene.render.engine = "CYCLES"
        scene.cycles.device = "CPU"
        scene.render.threads_mode = "FIXED"
        scene.render.threads = options.threads
        scene.cycles.samples = options.samples
        scene.cycles.seed = options.seed
        scene.cycles.use_animated_seed = False
        scene.cycles.use_adaptive_sampling = False
        scene.cycles.use_denoising = False
        scene.cycles.caustics_refractive = True
        scene.cycles.caustics_reflective = False
        scene.cycles.max_bounces = 12
        scene.cycles.transmission_bounces = 8
        scene.cycles.diffuse_bounces = 4
        scene.cycles.sample_clamp_indirect = 0
        scene.render.resolution_x = options.width
        scene.render.resolution_y = options.width * 9 // 16
        scene.render.resolution_percentage = 100
        scene.render.image_settings.file_format = "OPEN_EXR"
        scene.render.image_settings.color_depth = "32"
        scene.render.image_settings.color_mode = "RGB"
        scene.render.image_settings.exr_codec = "ZIP"
        scene.view_settings.view_transform = "Standard"
        scene.view_settings.exposure = 0
        scene.unit_settings.system = "METRIC"
        scene.unit_settings.scale_length = 1 / self.scale

    @staticmethod
    def material(name: str, node_type: str) -> tuple[bpy.types.Material, bpy.types.Node]:
        """Create a surface with the named stock BSDF and return the material and editable node."""
        material = bpy.data.materials.new(name)
        material.use_nodes = True
        tree = material.node_tree
        tree.nodes.clear()
        node = tree.nodes.new(node_type)
        output = tree.nodes.new("ShaderNodeOutputMaterial")
        tree.links.new(node.outputs[0], output.inputs["Surface"])
        return material, node

    def add_casters(self) -> None:
        """Load both precomputed interfaces, preserving their normals, geometry and relative placement."""
        oil, glass = self.material("Oil / water effective interface", "ShaderNodeBsdfGlass")
        glass.inputs["IOR"].default_value = 1.403 / 1.333
        glass.inputs["Roughness"].default_value = 0
        glass.inputs["Color"].default_value = (1, 1, 1, 1)
        for slide in ("rear", "front"):
            filename = slide + ("-flat" if self.options.shape == "flat" else "") + ".ply"
            bpy.ops.wm.ply_import(filepath=str((self.options.geometry / filename).resolve()),
                                  forward_axis="Y", up_axis="Z", global_scale=self.scale)
            obj = bpy.context.active_object
            obj.name = slide
            obj.data.materials.clear()
            obj.data.materials.append(oil)
            # MNEE requires the smooth-normal flag on every broad interface.
            # Parallel faces in the flat PLY already have constant vertex normals.
            for face in obj.data.polygons:
                face.use_smooth = abs(face.normal.z) > 0.0001
            obj.visible_camera = False
            obj.cycles.is_caustics_caster = True
            print("TSS mesh", slide, len(obj.data.vertices), len(obj.data.polygons),
                  "custom normals", obj.data.has_custom_normals, flush=True)

    def add_receiver_and_lamp(self) -> None:
        """Place the diffuse screen, finite-radius point lamp and orthographic observation camera."""
        bpy.ops.mesh.primitive_plane_add(size=0.60 * self.scale, location=(0, 0, -0.035 * self.scale))
        receiver = bpy.context.active_object
        receiver.name = "Diffuse receiver, observed from +Z"
        receiver.cycles.is_caustics_receiver = True
        paper, diffuse = self.material("White projection screen", "ShaderNodeBsdfDiffuse")
        diffuse.inputs["Color"].default_value = (1, 1, 1, 1)
        receiver.data.materials.append(paper)

        bpy.ops.object.light_add(type="POINT", location=(0, 0, 0.25 * self.scale))
        light = bpy.context.active_object.data
        # Inverse-square irradiance remains invariant under uniform scene scaling.
        light.energy = 2 * self.scale ** 2
        light.shadow_soft_size = 0.0007 * self.scale
        light.cycles.is_caustics_light = not self.options.no_shadow_caustics

        bpy.ops.object.camera_add(location=(0, 0, 0.15 * self.scale), rotation=(0, 0, 0))
        self.scene.camera = bpy.context.active_object
        camera = self.scene.camera.data
        camera.type = "ORTHO"
        camera.ortho_scale = self.scale * 0.16 * (0.25 + 0.035) / (0.25 - 0.00175)
        camera.clip_start = 0.001 * self.scale
        camera.clip_end = self.scale

    def render(self) -> None:
        """Assemble and optionally save the scene, then invoke native Cycles to write the EXR."""
        self.configure()
        if not self.options.empty_beam:
            self.add_casters()
        self.add_receiver_and_lamp()
        self.scene.render.filepath = str(self.options.output.resolve())
        if self.options.save_scene:
            bpy.ops.wm.save_as_mainfile(filepath=str(self.options.save_scene.resolve()), compress=True)
        bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    ReceiverStudy.from_cli().render()

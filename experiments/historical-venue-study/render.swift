import AppKit
import Metal
import SceneKit

// Offscreen review frame of the USD scene. Dimension values are metres.
guard CommandLine.arguments.count == 3 else {
    fatalError("Usage: swift render.swift scene.usdz review.png|stereo-directory")
}
let scene = try SCNScene(url: URL(fileURLWithPath: CommandLine.arguments[1]))
guard let device = MTLCreateSystemDefaultDevice() else { fatalError("Metal device unavailable") }
let renderer = SCNRenderer(device: device, options: nil)
renderer.scene = scene
let background = NSColor(srgbRed: 0.048, green: 0.072, blue: 0.092, alpha: 1)
scene.background.contents = background
let screenNode = scene.rootNode.childNode(withName: "Vinyl_30ft_by_20ft", recursively: true)
guard let screen = screenNode, screen.geometry != nil else { fatalError("USD vinyl screen geometry missing") }
// SceneKit can convert USD stage units on load. Recover its world units per metre.
let lowerLeft = screen.convertPosition(SCNVector3(-4.572,0,0), to:nil)
let lowerRight = screen.convertPosition(SCNVector3(4.572,0,0), to:nil)
let units = (lowerRight.x-lowerLeft.x)/9.144
func point(_ x: CGFloat, _ y: CGFloat, _ z: CGFloat) -> SCNVector3 {
    SCNVector3(x*units, y*units, z*units)
}
let camera = SCNCamera()
camera.usesOrthographicProjection = true
camera.orthographicScale = 7.35*Double(units)
camera.zNear = 0.01*Double(units)
camera.zFar = 100*Double(units)
let eye = SCNNode()
eye.camera = camera
eye.position = point(13.7,12.8,-12)
eye.look(at:point(0,2.45,-3.1))
scene.rootNode.addChildNode(eye)
renderer.pointOfView = eye
func light(_ type: SCNLight.LightType, _ intensity: CGFloat, _ at: SCNVector3) {
    let emitter = SCNLight()
    emitter.type = type
    emitter.intensity = intensity
    let node = SCNNode()
    node.light = emitter
    node.position = at
    if type == .directional { node.look(at:point(0,1,-3)) }
    scene.rootNode.addChildNode(node)
}
light(.ambient,480,point(0,0,0))
light(.directional,1250,point(-7,16,-11))
light(.directional,460,point(12,5,5))
if CommandLine.arguments[2].hasSuffix(".png") {
let imageSize = CGSize(width:1344,height:676)
let still = renderer.snapshot(atTime:0, with:imageSize, antialiasingMode:.multisampling2X)
let canvas = NSImage(size:NSSize(width:1440,height:952))
canvas.lockFocus()
let bounds = NSRect(origin:.zero,size:canvas.size)
background.setFill()
bounds.fill()
func text(_ message:String, _ x:CGFloat, _ y:CGFloat, _ size:CGFloat, _ weight:NSFont.Weight, _ color:NSColor) {
    (message as NSString).draw(at:NSPoint(x:x,y:y),withAttributes:[
        .font:NSFont.systemFont(ofSize:size,weight:weight),.foregroundColor:color])
}
let white = NSColor(white:0.94,alpha:1)
let quiet = NSColor(srgbRed:0.68,green:0.75,blue:0.78,alpha:1)
let gold = NSColor(srgbRed:0.96,green:0.76,blue:0.45,alpha:1)
still.draw(in:NSRect(x:48,y:157,width:1344,height:676))
text("Fillmore East",48,873,40,.medium,white)
text("Joshua Light Show   /   Rear-projection scale study",50,838,22,.regular,quiet)
text("BACKSTAGE VIEW",1191,870,15,.semibold,gold)
let rule = NSBezierPath()
rule.move(to:NSPoint(x:48,y:148)); rule.line(to:NSPoint(x:1392,y:148))
quiet.withAlphaComponent(0.5).setStroke(); rule.stroke()
text("VINYL SCREEN",48,105,16,.semibold,gold)
text("30 ft wide × 20 ft high",48,71,23,.medium,white)
text("9.144 × 6.096 metres",48,39,17,.regular,quiet)
text("REAR SYSTEM",457,105,16,.semibold,gold)
text("About 20 ft behind the stage",457,71,23,.medium,white)
text("Two elevated platforms. Lumia on the upper deck.",457,39,17,.regular,quiet)
text("ILLUSTRATIVE",976,105,16,.semibold,gold)
text("Equipment placement and size",976,71,23,.medium,white)
text("Stage datum, platform heights and screen image.",976,39,17,.regular,quiet)
canvas.unlockFocus()
guard let tiff = canvas.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data:tiff),
      let png = bitmap.representation(using:.png,properties:[:]) else { fatalError("PNG encoding failed") }
try png.write(to:URL(fileURLWithPath:CommandLine.arguments[2]))
print("Rendered using \(device.name); \(bitmap.pixelsWide) × \(bitmap.pixelsHigh); USD metres at SceneKit scale \(units).")
} else {
    let out = URL(fileURLWithPath:CommandLine.arguments[2], isDirectory:true)
    let width = 1920
    let height = 1080
    let baseline: CGFloat = 0.064*units
    let centerPosition = eye.position
    let right = eye.convertVector(SCNVector3(1,0,0), to:nil)
    let convergence = -eye.convertPosition(point(0,3.948,-1.2), from:nil).z
    let verticalFov: CGFloat = 42
    let fy = 1/tan(verticalFov * .pi / 360), fx = fy*CGFloat(height)/CGFloat(width)
    let near = 0.10*units, far = 100*units
    var projected = [String:[[Double]]]()
    let markers = [
        ("screenCenter", point(0,3.948,-1.2)),
        ("screenTopLeft", point(-4.572,6.996,-1.2)),
        ("screenBottomRight", point(4.572,0.9,-1.2)),
        ("lowerDeckRight", point(4.45,1.14,-6.921)),
        ("upperDeckLeft", point(-2.6,3.19,-8.675)),
        ("stageFront", point(-5.8,-0.01,3.2))
    ]
    camera.usesOrthographicProjection = false
    for (label, offset) in [("left", -baseline/2), ("right", baseline/2)] {
        eye.position = SCNVector3(centerPosition.x + offset*right.x, centerPosition.y + offset*right.y,
                                 centerPosition.z + offset*right.z)
        // Both eye rotations stay identical. Move the principal point in the image plane.
        var p = SCNMatrix4Identity
        p.m11 = fx; p.m22 = fy; p.m31 = -fx*offset/convergence
        p.m33 = -(far+near)/(far-near); p.m34 = -1; p.m43 = -2*far*near/(far-near); p.m44 = 0
        camera.projectionTransform = p
        let image = renderer.snapshot(atTime:0, with:CGSize(width:width,height:height),antialiasingMode:.multisampling2X)
        guard let tiff=image.tiffRepresentation, let bitmap=NSBitmapImageRep(data:tiff),
              bitmap.pixelsWide == width && bitmap.pixelsHigh == height,
              let png=bitmap.representation(using:.png,properties:[:]) else { fatalError("Expected a 1920 × 1080 eye") }
        try png.write(to:out.appendingPathComponent("fillmore-east-"+label+".png"))
        for (name, world) in markers {
            let v = eye.convertPosition(world, from:nil)
            let x = CGFloat(width)/2 * (1+fx*v.x / -v.z - p.m31)
            let y = CGFloat(height)/2 * (1-fy*v.y / -v.z)
            projected[name, default:[]].append([x,y].map(Double.init))
        }
    }
    let disparities = projected.mapValues { [ "left":$0[0], "right":$0[1], "rightMinusLeftPx":[$0[1][0]-$0[0][0],$0[1][1]-$0[0][1]] ] }
    let vertical = disparities.values.map { abs($0["rightMinusLeftPx"]![1]) }.max()!
    let screenDisparity = abs(disparities["screenCenter"]!["rightMinusLeftPx"]![0])
    guard vertical < 0.001 && screenDisparity < 0.001 else { fatalError("Stereo control points out of tolerance") }
    let report:[String:Any] = ["eyeSize":[width,height],"baselineM":0.064,"verticalFovDegrees":verticalFov,
                               "convergenceDistanceM":convergence/units,"convergenceNode":"screen center",
                               "method":"parallel cameras with off-axis projection in eye-local X","device":device.name,
                               "markersTopLeftPixels":disparities]
    let json = try JSONSerialization.data(withJSONObject:report,options:[.prettyPrinted,.sortedKeys])
    try json.write(to:out.appendingPathComponent("stereo.json"))
    print("Rendered parallel stereo 1920 × 1080 per eye. Maximum marker vertical disparity \(vertical) px. Screen center horizontal disparity \(screenDisparity) px.")
}

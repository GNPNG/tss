import Foundation
import Metal

// Tests Metal compute execution and readback for each enumerated device.
// This is a capability probe, not a fluid solver or performance benchmark.
let source = """
#include <metal_stdlib>
using namespace metal;
kernel void test(device uint *out [[buffer(0)]], uint i [[thread_position_in_grid]]) {
    if (i < 1024) out[i] = i * 3u + 7u;
}
"""
let devices = MTLCopyAllDevices()
if devices.isEmpty { fputs("No Metal devices found\n", stderr); exit(1) }
var failed = false
for device in devices {
    do {
        let library = try device.makeLibrary(source: source, options: nil)
        guard let function = library.makeFunction(name: "test"),
              let queue = device.makeCommandQueue(),
              let buffer = device.makeBuffer(length: 1024 * MemoryLayout<UInt32>.stride, options: .storageModeShared),
              let command = queue.makeCommandBuffer(),
              let encoder = command.makeComputeCommandEncoder() else { throw NSError(domain: "Metal probe allocation", code: 1) }
        let pipeline = try device.makeComputePipelineState(function: function)
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(buffer, offset: 0, index: 0)
        encoder.dispatchThreads(MTLSize(width: 1024, height: 1, depth: 1),
                                threadsPerThreadgroup: MTLSize(width: min(256, pipeline.maxTotalThreadsPerThreadgroup), height: 1, depth: 1))
        encoder.endEncoding()
        command.commit()
        command.waitUntilCompleted()
        if let error = command.error { throw error }
        let values = buffer.contents().bindMemory(to: UInt32.self, capacity: 1024)
        let valid = (0..<1024).allSatisfy { values[$0] == UInt32($0 * 3 + 7) }
        if !valid { failed = true }
        print("name=\(device.name); registryID=\(device.registryID); lowPower=\(device.isLowPower); removable=\(device.isRemovable); metal3=\(device.supportsFamily(.metal3)); maxBufferLength=\(device.maxBufferLength); maxThreadsPerGroup=\(pipeline.maxTotalThreadsPerThreadgroup); computeReadback=\(valid ? "PASS" : "FAIL")")
    } catch {
        failed = true
        print("name=\(device.name); computeReadback=FAIL; error=\(error)")
    }
}
if failed { exit(1) }

#include <metal_stdlib>
using namespace metal;

kernel void gpu_load(device float *data [[buffer(0)]],
                     constant uint &iters [[buffer(1)]],
                     uint id [[thread_position_in_grid]]) {
    float x = data[id] + float(id % 997) * 0.001;
    for (uint i = 0; i < iters; i++) {
        x = sin(x) * cos(x * 1.31337) + sqrt(fabs(x) + 1.0);
    }
    data[id] = x;
}

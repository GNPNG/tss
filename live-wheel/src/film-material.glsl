// Two halves hold the rear and front material properties at their own optical
// ray intersections. The texture is in visible-film coordinates.
void main() {
    bool front = fieldUV.y >= 0.5;
    vec2 uv = vec2(fieldUV.x, fract(2.0*fieldUV.y));
    vec2 distance = distanceAt(uv);
    vec2 depth = gapDepth(uv,broadCoverage(uv));
    float d = front ? distance.g : distance.r;
    vec2 normal = vec2(0.0);
    float rim = 0.0;
    if (abs(d) < 18.0) {
        vec2 stepUV = 1.5/slideUnits;
        vec2 gx = distanceAt(uv+vec2(stepUV.x,0))-distanceAt(uv-vec2(stepUV.x,0));
        vec2 gy = distanceAt(uv+vec2(0,stepUV.y))-distanceAt(uv-vec2(0,stepUV.y));
        normal = unitOrZero(front ? vec2(gx.g,gy.g) : vec2(gx.r,gy.r));
        vec2 resolved = resolvedRim(uv);
        rim = front ? resolved.g : resolved.r;
    }
    result = vec4(normal,rim,front ? depth.g : depth.r);
}

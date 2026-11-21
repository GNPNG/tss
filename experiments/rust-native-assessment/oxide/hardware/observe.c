// Test instrumentation only. Use SDK declarations to read properties from the
// exact compression session that encoded the frames, after completion.
#include <VideoToolbox/VideoToolbox.h>
#include <stdio.h>
#include <stdbool.h>

static bool hardware_confirmed = false;
bool tss_hardware_confirmed(void) { return hardware_confirmed; }

void tss_observe_session(VTCompressionSessionRef session) {
    CFTypeRef value = NULL;
    OSStatus status = VTSessionCopyProperty(session,
        kVTCompressionPropertyKey_UsingHardwareAcceleratedVideoEncoder, NULL, &value);
    hardware_confirmed = status == noErr && value &&
        CFGetTypeID(value) == CFBooleanGetTypeID() && CFBooleanGetValue((CFBooleanRef)value);
    fprintf(stderr, "hardware_property_osstatus=%d\nusing_hardware_encoder=%s\n",
        (int)status, hardware_confirmed ? "true" : "false");
    if (value) CFRelease(value);

    value = NULL;
    status = VTSessionCopyProperty(session, kVTCompressionPropertyKey_EncoderID, NULL, &value);
    char name[512];
    if (status == noErr && value && CFGetTypeID(value) == CFStringGetTypeID() &&
        CFStringGetCString((CFStringRef)value, name, sizeof(name), kCFStringEncodingUTF8))
        fprintf(stderr, "encoder_id=%s\n", name);
    else
        fprintf(stderr, "encoder_id_unavailable_osstatus=%d\n", (int)status);
    if (value) CFRelease(value);
}

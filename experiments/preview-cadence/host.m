// A single visible WKWebView visits the two editor snapshots in sequence.
#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>
@interface ProbeHost : NSObject <WKScriptMessageHandler, WKNavigationDelegate>
@property(nonatomic,strong) WKWebView *view;
@property(nonatomic,copy) NSString *origin;
@property(nonatomic) BOOL second;
@property(nonatomic) int result;
@end
@implementation ProbeHost
- (void)finish {
    [NSApp stop:nil];
    // WebKit messages arrive outside an NSEvent. Wake nextEventMatchingMask so run returns.
    [NSApp postEvent:[NSEvent otherEventWithType:NSEventTypeApplicationDefined location:NSZeroPoint
        modifierFlags:0 timestamp:0 windowNumber:0 context:nil subtype:0 data1:0 data2:0] atStart:NO];
}
- (void)failed:(NSString *)reason {
    fprintf(stderr, "%s\n", reason.UTF8String);
    self.result=1;
    [self finish];
}
- (void)webViewWebContentProcessDidTerminate:(WKWebView *)view {
    [self failed:@"WebKit web content process terminated"];
}
- (void)webView:(WKWebView *)view didFailProvisionalNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    [self failed:error.localizedDescription];
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)msg {
    if (![msg.body isKindOfClass:NSString.class]) { [self failed:@"Invalid measurement"]; return; }
    const char *body=[msg.body UTF8String];
    if (strncmp(body, "ERROR ",6)==0) { [self failed:msg.body]; return; }
    puts(body); fflush(stdout);
    if (self.second) { [self finish]; return; }
    self.second=YES;
    [self.view loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:[self.origin stringByAppendingString:@"/working/index.html"]]]];
}
@end
int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc!=2) return 2;
        NSApplication *app=[NSApplication sharedApplication];
        [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
        ProbeHost *host=[ProbeHost new];
        host.origin=[NSString stringWithUTF8String:argv[1]];
        WKWebViewConfiguration *config=[WKWebViewConfiguration new];
        config.websiteDataStore=WKWebsiteDataStore.nonPersistentDataStore;
        [config.userContentController addScriptMessageHandler:host name:@"cadence"];
        NSRect frame=NSMakeRect(80,90,1000,740);
        NSWindow *window=[[NSWindow alloc] initWithContentRect:frame styleMask:NSWindowStyleMaskTitled
                                         backing:NSBackingStoreBuffered defer:NO];
        window.title=@"The Sacred Swirls • preview cadence";
        window.releasedWhenClosed=NO;
        host.view=[[WKWebView alloc] initWithFrame:frame configuration:config];
        host.view.navigationDelegate=host;
        [window setContentView:host.view];
        [window makeKeyAndOrderFront:nil];
        [app activateIgnoringOtherApps:YES];
        [host.view loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:[host.origin stringByAppendingString:@"/baseline/index.html"]]]];
        [app run];
        [host.view stopLoading];
        [window orderOut:nil];
        [config.userContentController removeScriptMessageHandlerForName:@"cadence"];
        return host.result;
    }
}

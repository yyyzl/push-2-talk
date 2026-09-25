// Native acceptance of the real event consumer. Creates in-memory CGEvents only;
// never posts events, activates windows, records audio, or changes permissions.
#import <ApplicationServices/ApplicationServices.h>
#import <Foundation/Foundation.h>
@interface TestAXNode : NSObject
@property(nonatomic,strong) NSDictionary *attributes;
@end
@implementation TestAXNode
@end
static TestAXNode *node(NSDictionary *attributes) {
    TestAXNode *result=[TestAXNode new]; result.attributes=attributes; return result;
}
static TestAXNode *testSystem;
static AXUIElementRef testSystemWide(void) { return (AXUIElementRef)CFBridgingRetain(testSystem); }
static AXError testMessagingTimeout(AXUIElementRef element,float timeout) {
    (void)element; (void)timeout; return kAXErrorSuccess;
}
static AXError testCopyAttribute(AXUIElementRef element,CFStringRef key,CFTypeRef *value) {
    id object=(__bridge id)element;
    if (![object isKindOfClass:TestAXNode.class]) return kAXErrorInvalidUIElement;
    id result=((TestAXNode *)object).attributes[(__bridge NSString *)key];
    if (!result) return kAXErrorAttributeUnsupported;
    *value=CFBridgingRetain(result); return kAXErrorSuccess;
}
static bool testPostAllowed = false;
static unsigned testPostedEvents = 0;
static bool testTrusted(void) { return true; }
static bool testPostAccess(void) { return testPostAllowed; }
static void testPost(CGEventTapLocation location, CGEventRef event) {
    (void)location; (void)event; testPostedEvents++;
}
static bool testApplicationActive, testElementFocused;
static AXError testSetAttribute(AXUIElementRef element, CFStringRef key, CFTypeRef value) {
    (void)element; (void)value;
    if (CFEqual(key,kAXFrontmostAttribute)) testApplicationActive=true;
    if (CFEqual(key,kAXFocusedAttribute)) {
        testElementFocused=testApplicationActive;
        return testElementFocused ? kAXErrorSuccess : kAXErrorCannotComplete;
    }
    return kAXErrorSuccess;
}
static AXError testTabActionError=kAXErrorSuccess;
static AXError testAction(AXUIElementRef element, CFStringRef action) {
    (void)element; return CFEqual(action,kAXPressAction) ? testTabActionError : kAXErrorSuccess;
}
#define AXIsProcessTrusted testTrusted
#define CGPreflightPostEventAccess testPostAccess
#define CGEventPost testPost
#define AXUIElementSetAttributeValue testSetAttribute
#define AXUIElementPerformAction testAction
#define AXUIElementCopyAttributeValue testCopyAttribute
#define AXUIElementCreateSystemWide testSystemWide
#define AXUIElementSetMessagingTimeout testMessagingTimeout
#import "../src/platform/macos/native.m"
#include <assert.h>

static void resetFixture(void) {
    memset(keyboardState,0,sizeof(keyboardState));
    keyboardHead=keyboardCount=0; keyboardReset=false;
}
static void key(CGKeyCode code,bool down,bool repeat) {
    CGEventRef event=CGEventCreateKeyboardEvent(NULL,code,down);
    CGEventSetIntegerValueField(event,kCGKeyboardEventAutorepeat,repeat);
    observeKeyboard(NULL,down?kCGEventKeyDown:kCGEventKeyUp,event,NULL);
    CFRelease(event);
}
int main(void) { @autoreleasepool {
    TestAXNode *original=node(@{@"AXValue":@YES}), *other=node(@{@"AXValue":@NO});
    TestAXNode *group=node(@{@"AXRole":@"AXTabGroup",@"AXTabs":@[original,other]});
    TestAXNode *originalWindow=node(@{@"AXChildren":@[group]});
    assert(same(selectedWindowTab(originalWindow),original));
    puts("PASS capture binds the selected native document tab");
    original.attributes=@{@"AXValue":@NO}; other.attributes=@{@"AXValue":@YES};
    TestAXNode *visibleWindow=node(@{@"AXChildren":@[group]});
    assert(same(windowContainingTab(@[visibleWindow],original),visibleWindow));
    assert(same(selectedWindowTab(visibleWindow),other));
    puts("PASS an inactive tab remains live even when its original window is absent");
    group.attributes=@{@"AXRole":@"AXTabGroup",@"AXTabs":@[other]};
    assert(windowContainingTab(@[visibleWindow],original)==nil);
    TestAXNode *lookalike=node(@{@"AXValue":@NO});
    group.attributes=@{@"AXRole":@"AXTabGroup",@"AXTabs":@[lookalike,other]};
    assert(windowContainingTab(@[visibleWindow],original)==nil);
    puts("PASS a closed tab cannot match a different document with identical attributes");
    TestAXNode *fullElement=node(@{});
    TestAXNode *fullWindow=node(@{@"AXFullScreen":@YES,@"AXChildren":@[]});
    TestAXNode *fullApp=node(@{@"AXWindows":@[fullWindow],@"AXFocusedWindow":fullWindow,@"AXFocusedUIElement":fullElement});
    PTTTarget *fullTarget=[PTTTarget new];
    fullTarget.pid=getpid(); fullTarget.application=fullApp;
    fullTarget.window=fullWindow; fullTarget.element=fullElement; fullTarget.tab=original;
    testSystem=node(@{@"AXFocusedApplication":fullApp});
    assert(valid(fullTarget));
    assert(focused(fullTarget));
    puts("PASS fullscreen may hide native tabs while the exact original input remains selected");
    testSystem.attributes=@{@"AXFocusedApplication":node(@{})};
    assert(valid(fullTarget) && !focused(fullTarget));
    testSystem.attributes=@{@"AXFocusedApplication":fullApp};
    puts("PASS fullscreen still requires the original application to own global focus");
    fullApp.attributes=@{@"AXWindows":@[fullWindow],@"AXFocusedWindow":fullWindow,@"AXFocusedUIElement":node(@{})};
    assert(!valid(fullTarget));
    fullApp.attributes=@{@"AXWindows":@[],@"AXFocusedWindow":fullWindow,@"AXFocusedUIElement":fullElement};
    assert(!valid(fullTarget));
    fullApp.attributes=@{@"AXWindows":@[fullWindow],@"AXFocusedWindow":node(@{}),@"AXFocusedUIElement":fullElement};
    assert(!valid(fullTarget));
    puts("PASS fullscreen rejects a replaced input, missing original window or different focused window");
    fullApp.attributes=@{@"AXWindows":@[fullWindow],@"AXFocusedWindow":fullWindow,@"AXFocusedUIElement":fullElement};
    fullWindow.attributes=@{@"AXFullScreen":@NO,@"AXChildren":@[]};
    assert(!valid(fullTarget));
    fullWindow.attributes=@{@"AXFullScreen":@YES,@"AXChildren":@[group]};
    assert(!valid(fullTarget));
    puts("PASS a missing tab in ordinary mode or a visible replacement tab still fails closed");
    PTTTarget *restoreTarget=[PTTTarget new];
    restoreTarget.application=@"application"; restoreTarget.window=@"window"; restoreTarget.element=@"element";
    restoreTarget.tab=original; testTabActionError=kAXErrorInvalidUIElement;
    restoreNativeTarget(restoreTarget);
    assert(!testApplicationActive && !testElementFocused);
    assert(restoreTarget.tabError==kAXErrorInvalidUIElement);
    puts("PASS a rejected tab switch never continues to activate or focus another document");
    restoreTarget.tab=nil; testTabActionError=kAXErrorSuccess;
    restoreNativeTarget(restoreTarget);
    assert(testApplicationActive && testElementFocused);
    assert(restoreTarget.restoreRequested && restoreTarget.focusError==kAXErrorSuccess);
    puts("PASS restoration activates the target application before focusing its input element");
    NSString *fixture=[NSTemporaryDirectory() stringByAppendingPathComponent:[NSString stringWithFormat:@"PushToTalk ATDD %@.txt",NSUUID.UUID.UUIDString]];
    assert([@"Test fixture" writeToFile:fixture atomically:YES encoding:NSUTF8StringEncoding error:nil]);
    NSString *document=[NSURL fileURLWithPath:fixture].absoluteString;
    assert(atddFixtureMatches(@"com.apple.TextEdit",document,@"AXTextArea",[fixture stringByResolvingSymlinksInPath]));
    assert(!atddFixtureMatches(@"com.openai.codex",document,@"AXTextArea",fixture));
    assert(!atddFixtureMatches(@"com.apple.TextEdit",@"file:///tmp/user-document.txt",@"AXTextArea",fixture));
    assert(!atddFixtureMatches(@"com.apple.TextEdit",document,@"AXMenu",fixture));
    assert(!atddFixtureMatches(@"com.apple.TextEdit",nil,@"AXTextArea",fixture));
    assert([NSFileManager.defaultManager removeItemAtPath:fixture error:nil]);
    puts("PASS ATDD only accepts its own TextEdit fixture, never another app, document or menu");

    assert(!ptt_send_shortcut(9));
    assert(testPostedEvents==0);
    puts("PASS denied event posting cannot be reported as a successful paste");
    testPostAllowed=true;
    assert(ptt_send_shortcut(9));
    assert(testPostedEvents==2);
    puts("PASS permitted shortcut dispatches one down/up pair to the test sink");

    resetFixture();
    key(120,true,false); key(120,false,false);
    assert(keyboardCount==2);
    assert(keyboardQueue[0][120]==1 && keyboardQueue[1][120]==0);
    puts("PASS native short press preserves ordered down/up between consumer polls");

    resetFixture();
    key(120,true,false); key(120,true,true); key(120,true,false);
    assert(keyboardCount==1);
    puts("PASS autorepeat and duplicate key-down do not retrigger");

    resetFixture();
    CGEventRef event=CGEventCreate(NULL);
    CGEventSetIntegerValueField(event,kCGKeyboardEventKeycode,59);
    CGEventSetFlags(event,kCGEventFlagMaskControl|NX_DEVICELCTLKEYMASK);
    observeKeyboard(NULL,kCGEventFlagsChanged,event,NULL);
    assert(keyboardState[59]==1 && keyboardState[62]==0);
    CGEventSetFlags(event,0);
    observeKeyboard(NULL,kCGEventFlagsChanged,event,NULL);
    assert(keyboardState[59]==0 && keyboardCount==2);
    CFRelease(event);
    puts("PASS modifier flags preserve left/right and release edges");

    resetFixture();
    for (int i=0;i<256;i++) key(120,i%2==0,false);
    assert(keyboardCount==256 && !keyboardReset);
    key(120,true,false);
    assert(keyboardReset && keyboardCount==0);
    key(120,false,false); key(120,true,false);
    assert(keyboardCount==0);
    puts("PASS overflow discards stale starts and suppresses replay until reset consumed");
    return 0;
} }

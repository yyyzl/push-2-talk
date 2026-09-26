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
static unsigned testSelectionReadDelay;
static id testPendingSelection;
static AXUIElementRef testSystemWide(void) { return (AXUIElementRef)CFBridgingRetain(testSystem); }
static AXError testMessagingTimeout(AXUIElementRef element,float timeout) {
    (void)element; (void)timeout; return kAXErrorSuccess;
}
static AXError testCopyAttribute(AXUIElementRef element,CFStringRef key,CFTypeRef *value) {
    id object=(__bridge id)element;
    if (![object isKindOfClass:TestAXNode.class]) return kAXErrorInvalidUIElement;
    if (CFEqual(key,kAXSelectedTextRangeAttribute) && testPendingSelection && testSelectionReadDelay--==0) {
        NSMutableDictionary *attributes=((TestAXNode *)object).attributes.mutableCopy;
        attributes[(__bridge NSString *)key]=testPendingSelection;
        ((TestAXNode *)object).attributes=attributes;
        testPendingSelection=nil;
    }
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
static AXError testSelectionError=kAXErrorSuccess;
static bool testSelectionApplied=true;
static unsigned testSelectionWrites=0;
static AXError testSetAttribute(AXUIElementRef element, CFStringRef key, CFTypeRef value) {
    if (CFEqual(key,kAXSelectedTextRangeAttribute)) {
        testSelectionWrites++;
        if (testSelectionError!=kAXErrorSuccess) return testSelectionError;
        if (testSelectionApplied) {
            if (testSelectionReadDelay) {
                testPendingSelection=(__bridge id)value;
                return kAXErrorSuccess;
            }
            TestAXNode *object=(__bridge TestAXNode *)element;
            NSMutableDictionary *attributes=object.attributes.mutableCopy;
            attributes[(__bridge NSString *)key]=(__bridge id)value;
            object.attributes=attributes;
        }
        return kAXErrorSuccess;
    }
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

    TestAXNode *browserOriginal=node(@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXTitle":@"Same title",@"AXValue":@YES});
    TestAXNode *browserOther=node(@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXValue":@NO});
    TestAXNode *pageTab=node(@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXValue":@YES});
    TestAXNode *pageGroup=node(@{@"AXRole":@"AXTabGroup",@"AXTabs":@[pageTab]});
    TestAXNode *webArea=node(@{@"AXRole":@"AXWebArea",@"AXChildren":@[pageGroup]});
    TestAXNode *tabContainer=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[browserOriginal,browserOther]});
    TestAXNode *tabScroll=node(@{@"AXRole":@"AXScrollArea",@"AXChildren":@[tabContainer]});
    TestAXNode *browserStrip=node(@{@"AXRole":@"AXTabGroup",@"AXChildren":@[node(@{@"AXRole":@"AXButton"}),tabScroll]});
    TestAXNode *browserRoot=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[webArea,browserStrip]});
    TestAXNode *browserWindow=node(@{@"AXChildren":@[browserRoot]});
    assert(same(selectedWindowTab(browserWindow),browserOriginal));
    assert(windowTabs(browserWindow).count==2);
    puts("PASS browser window tabs are found through native containers outside web content");
    browserOriginal.attributes=@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXTitle":@"Same title",@"AXValue":@NO};
    browserOther.attributes=@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXValue":@YES};
    assert(same(selectedWindowTab(browserWindow),browserOther));
    assert(same(windowContainingTab(@[browserWindow],browserOriginal),browserWindow));
    puts("PASS switching browser tabs preserves the captured original tab identity");
    TestAXNode *browserLookalike=node(browserOriginal.attributes);
    tabContainer.attributes=@{@"AXRole":@"AXGroup",@"AXChildren":@[browserLookalike,browserOther]};
    assert(windowContainingTab(@[browserWindow],browserOriginal)==nil);
    puts("PASS a closed browser tab cannot match a replacement with the same title and properties");
    browserRoot.attributes=@{@"AXRole":@"AXGroup",@"AXChildren":@[webArea,node(@{@"AXRole":@"AXButton",@"AXChildren":@[browserStrip]})]};
    assert(windowTabs(browserWindow).count==0);
    puts("PASS page tabs and descendants of non-container controls never become window targets");
    // Observed Chrome vertical strip: window / group x4 / tab group / group /
    // scroll area / group / group / radio button (depth 10 from AXWindow).
    TestAXNode *deepTabs=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[browserOriginal,browserOther]});
    id deepScroll=node(@{@"AXRole":@"AXScrollArea",@"AXChildren":@[node(@{@"AXRole":@"AXGroup",@"AXChildren":@[deepTabs]})]});
    id deepStrip=node(@{@"AXRole":@"AXTabGroup",@"AXChildren":@[node(@{@"AXRole":@"AXGroup",@"AXChildren":@[deepScroll]})]});
    id deepRoot=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[webArea,deepStrip]});
    for (unsigned i=0;i<3;i++) deepRoot=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[deepRoot]});
    TestAXNode *deepWindow=node(@{@"AXChildren":@[deepRoot]});
    assert(same(selectedWindowTab(deepWindow),browserOther));
    assert(windowTabs(deepWindow).count==2 && same(windowContainingTab(@[deepWindow],browserOriginal),deepWindow));
    puts("PASS actual Chrome vertical tab depth preserves selection and original inactive identity");
    deepTabs.attributes=@{@"AXRole":@"AXGroup",@"AXChildren":@[browserLookalike,browserOther]};
    assert(windowContainingTab(@[deepWindow],browserOriginal)==nil);
    puts("PASS a closed deeply nested browser tab cannot match a replacement");
    for (unsigned i=0;i<32;i++) deepRoot=node(@{@"AXRole":@"AXGroup",@"AXChildren":@[deepRoot]});
    deepWindow.attributes=@{@"AXChildren":@[deepRoot]};
    assert(windowTabs(deepWindow).count==0);
    puts("PASS pathological depth still stops without selecting a speculative target");
    TestAXNode *cycle=node(@{@"AXRole":@"AXGroup"});
    cycle.attributes=@{@"AXRole":@"AXGroup",@"AXChildren":@[cycle]};
    TestAXNode *cycleWindow=node(@{@"AXChildren":@[cycle]});
    assert(windowTabs(cycleWindow).count==0);
    cycle.attributes=@{};
    puts("PASS malformed cyclic accessibility trees terminate without returning a target");
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
    NSString *browserFixture=[fixture stringByAppendingString:@".html"];
    NSString *browserDocument=[NSURL fileURLWithPath:browserFixture].absoluteString;
    assert(atddFixtureMatches(@"com.google.Chrome",browserDocument,@"AXTextArea",browserFixture));
    assert(!atddFixtureMatches(@"com.apple.TextEdit",browserDocument,@"AXTextArea",browserFixture));
    assert(!atddFixtureMatches(@"com.google.Chrome",document,@"AXTextArea",fixture));
    assert(!atddFixtureMatches(@"com.google.Chrome",browserDocument,@"AXTextField",browserFixture));
    NSString *remote=[browserDocument stringByReplacingOccurrencesOfString:@"file://" withString:@"https://example.com"];
    assert(!atddFixtureMatches(@"com.google.Chrome",remote,@"AXTextArea",browserFixture));
    assert(!atddFixtureMatches(@"com.apple.TextEdit",[document stringByReplacingOccurrencesOfString:@"file://" withString:@"https://example.com"],@"AXTextArea",fixture));
    assert(!atddFixtureMatches(@"com.google.Chrome",[browserDocument stringByReplacingOccurrencesOfString:@"file://" withString:@"file://other-host"],@"AXTextArea",browserFixture));
    puts("PASS browser ATDD is restricted to the exact local HTML textarea and known application");
    assert([NSFileManager.defaultManager removeItemAtPath:fixture error:nil]);
    puts("PASS ATDD only accepts its own TextEdit fixture, never another app, document or menu");

    NSString *fixtureText=@"PTT\n你好😀\n";
    TestAXNode *selectionElement=node(@{@"AXValue":fixtureText});
    TestAXNode *selectionWindow=node(@{});
    TestAXNode *selectionApp=node(@{@"AXWindows":@[selectionWindow],@"AXFocusedWindow":selectionWindow,@"AXFocusedUIElement":selectionElement});
    PTTTarget *selectionTarget=[PTTTarget new];
    selectionTarget.pid=getpid(); selectionTarget.application=selectionApp;
    selectionTarget.window=selectionWindow; selectionTarget.element=selectionElement;
    testSystem=node(@{@"AXFocusedApplication":selectionApp});
    assert(selectFixtureText(selectionTarget,fixtureText,CFRangeMake(4,4)));
    CFRange selected;
    assert(AXValueGetValue((__bridge AXValueRef)attribute(selectionElement,kAXSelectedTextRangeAttribute),kAXValueCFRangeType,&selected));
    assert(selected.location==4 && selected.length==4);
    puts("PASS ATDD selects the exact fixture range using UTF-16 offsets");
    assert(selectFixtureText(selectionTarget,fixtureText,CFRangeMake(fixtureText.length,0)));
    puts("PASS ATDD question mode clears selection at the fixture caret");
    unsigned writesBefore=testSelectionWrites;
    assert(!selectFixtureText(selectionTarget,@"some other document",CFRangeMake(0,1)));
    assert(!selectFixtureText(selectionTarget,fixtureText,CFRangeMake(-1,1)));
    assert(!selectFixtureText(selectionTarget,fixtureText,CFRangeMake(0,fixtureText.length+1)));
    testSystem.attributes=@{@"AXFocusedApplication":node(@{})};
    assert(!selectFixtureText(selectionTarget,fixtureText,CFRangeMake(0,1)));
    assert(testSelectionWrites==writesBefore);
    puts("PASS ATDD never selects unrelated, unfocused or out-of-range text");
    testSystem.attributes=@{@"AXFocusedApplication":selectionApp};
    testSelectionError=kAXErrorCannotComplete;
    assert(!selectFixtureText(selectionTarget,fixtureText,CFRangeMake(4,4)));
    testSelectionError=kAXErrorSuccess; testSelectionApplied=false;
    assert(!selectFixtureText(selectionTarget,fixtureText,CFRangeMake(4,4)));
    testSelectionApplied=true;
    puts("PASS ATDD rejects denied or unapplied selection updates");

    testSelectionReadDelay=2;
    writesBefore=testSelectionWrites;
    assert(selectFixtureText(selectionTarget,fixtureText,CFRangeMake(4,4)));
    assert(testPendingSelection==nil && testSelectionWrites==writesBefore+1);
    testSelectionReadDelay=0;
    puts("PASS ATDD verifies asynchronous selection without posting duplicate selection writes");

    // A probe must expose the native hierarchy without reading document contents.
    TestAXNode *diagnosticTab=node(@{@"AXRole":@"AXRadioButton",@"AXSubrole":@"AXTabButton",@"AXTitle":@"PRIVATE_TITLE",@"AXValue":@YES});
    TestAXNode *diagnosticWeb=node(@{@"AXRole":@"AXWebArea",@"AXValue":@"PRIVATE_BODY",@"AXChildren":@[node(@{@"AXRole":@"AXPrivatePageChild"})]});
    TestAXNode *diagnosticGroup=node(@{@"AXRole":@"AXTabGroup",@"AXChildren":@[diagnosticTab]});
    selectionWindow.attributes=@{@"AXChildren":@[diagnosticWeb,diagnosticGroup]};
    initializeTargets(); targets[@9999]=selectionTarget;
    char *diagnostic=ptt_atdd_target_description(9999);
    NSString *diagnosticText=[NSString stringWithUTF8String:diagnostic];
    ptt_free_string(diagnostic);
    assert([diagnosticText containsString:@"AXTabGroup"] && [diagnosticText containsString:@"AXTabButton"]);
    assert(![diagnosticText containsString:@"PRIVATE"] && ![diagnosticText containsString:@"AXPrivatePageChild"]);
    puts("PASS ATDD exposes native tab roles but excludes titles, values and web descendants");
    cycle.attributes=@{@"AXRole":@"AXGroup",@"AXChildren":@[cycle]};
    selectionWindow.attributes=@{@"AXChildren":@[cycle]};
    diagnostic=ptt_atdd_target_description(9999);
    assert(strlen(diagnostic)<4096);
    ptt_free_string(diagnostic); cycle.attributes=@{}; [targets removeObjectForKey:@9999];
    puts("PASS ATDD hierarchy diagnostics terminate on cyclic trees with bounded output");

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

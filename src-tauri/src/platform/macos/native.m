// Small C ABI bridge: AppKit objects and AX references stay on the native side.
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <AVFoundation/AVFoundation.h>
#import <Carbon/Carbon.h>
#import <IOKit/hidsystem/IOLLEvent.h>

@interface PTTTarget : NSObject
@property(nonatomic) pid_t pid;
@property(nonatomic, strong) id application;
@property(nonatomic, strong) id window;
@property(nonatomic, strong) id element;
@property(nonatomic, strong) id tab;
@property(nonatomic) BOOL restoreRequested;
@property(nonatomic, copy) NSString *activationResult;
@property(nonatomic) AXError tabError;
@property(nonatomic) AXError frontmostError;
@property(nonatomic) AXError raiseError;
@property(nonatomic) AXError mainError;
@property(nonatomic) AXError focusError;
@property(nonatomic, copy) NSString *fixtureSelectionStatus;
@end
@implementation PTTTarget
@end

// All AX calls are serialized and bounded by AX messaging timeouts. Target tokens are
// monotonic, never pointers/PIDs; eviction fails closed instead of reusing an old target.
static dispatch_queue_t targetQueue;
static NSMutableDictionary<NSNumber *, PTTTarget *> *targets;
static uint64_t nextTarget;
static void initializeTargets(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        targetQueue=dispatch_queue_create("com.pushtotalk.accessibility", DISPATCH_QUEUE_SERIAL);
        targets=[NSMutableDictionary new]; nextTarget=1;
    });
}
static id attribute(id object, CFStringRef key) {
    CFTypeRef value=NULL;
    if (!object || AXUIElementCopyAttributeValue((__bridge AXUIElementRef)object,key,&value)!=kAXErrorSuccess) return nil;
    return CFBridgingRelease(value);
}
static BOOL same(id left,id right) { return left && right && CFEqual((__bridge CFTypeRef)left,(__bridge CFTypeRef)right); }
static BOOL secure(id element) { return [attribute(element,kAXSubroleAttribute) isEqualToString:@"AXSecureTextField"]; }
// Chrome's window chrome nests its tab strip and may expose tab buttons through
// AXChildren instead of AXTabs. Walk native containers only: never AXWebArea or
// the descendants of arbitrary controls, even if a page exposes similar tabs.
static void collectWindowTabs(id node, NSUInteger depth, NSUInteger *remaining,
                              CFAbsoluteTime deadline, BOOL insideTabGroup, NSMutableArray *result) {
    // Chrome's vertical tabs reach depth 9 relative to the window's first child.
    // Allow native wrapper variation while retaining independent node/time caps.
    if (!node || depth>16 || *remaining==0 || result.count>=128 || CFAbsoluteTimeGetCurrent()>deadline) return;
    (*remaining)--;
    AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)node,0.05);
    NSString *role=attribute(node,kAXRoleAttribute);
    if (insideTabGroup && [role isEqualToString:(__bridge NSString *)kAXRadioButtonRole] &&
        [attribute(node,kAXSubroleAttribute) isEqualToString:@"AXTabButton"]) {
        [result addObject:node];
        return;
    }
    if ([role isEqualToString:(__bridge NSString *)kAXTabGroupRole]) {
        id tabs=attribute(node,kAXTabsAttribute);
        if ([tabs isKindOfClass:NSArray.class] && [tabs count]>0) {
            [result addObjectsFromArray:[tabs subarrayWithRange:NSMakeRange(0,MIN([tabs count],128-result.count))]];
            return;
        }
        insideTabGroup=YES;
    } else if (![role isEqualToString:(__bridge NSString *)kAXGroupRole] &&
               ![role isEqualToString:(__bridge NSString *)kAXSplitGroupRole] &&
               ![role isEqualToString:(__bridge NSString *)kAXScrollAreaRole]) {
        return;
    }
    id children=attribute(node,kAXChildrenAttribute);
    if (![children isKindOfClass:NSArray.class]) return;
    for (id child in [children subarrayWithRange:NSMakeRange(0,MIN([children count],128))]) {
        if (*remaining==0 || result.count>=128 || CFAbsoluteTimeGetCurrent()>deadline) break;
        collectWindowTabs(child,depth+1,remaining,deadline,insideTabGroup,result);
    }
}
// Track native window tabs by AX identity, never by a document title or position.
static NSArray *windowTabs(id window) {
    id children=attribute(window,kAXChildrenAttribute);
    if (![children isKindOfClass:NSArray.class]) return @[];
    NSMutableArray *result=[NSMutableArray new];
    NSUInteger remaining=256;
    CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+0.25;
    for (id child in [children subarrayWithRange:NSMakeRange(0,MIN([children count],32))]) {
        if (remaining==0 || result.count>=128 || CFAbsoluteTimeGetCurrent()>deadline) break;
        collectWindowTabs(child,0,&remaining,deadline,NO,result);
    }
    return result;
}
static id selectedWindowTab(id window) {
    NSArray *tabs=windowTabs(window);
    CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+0.25;
    for (id tab in [tabs subarrayWithRange:NSMakeRange(0,MIN(tabs.count,128))]) {
        if (CFAbsoluteTimeGetCurrent()>deadline) break;
        id selected=attribute(tab,kAXValueAttribute);
        if ([selected isKindOfClass:NSNumber.class] && [selected boolValue]) return tab;
    }
    return nil;
}
static id windowContainingTab(NSArray *windows,id tab) {
    if (!tab) return nil;
    CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+0.5;
    for (id window in [windows subarrayWithRange:NSMakeRange(0,MIN(windows.count,32))]) {
        if (CFAbsoluteTimeGetCurrent()>deadline) break;
        for (id candidate in windowTabs(window)) if (same(candidate,tab)) return window;
    }
    return nil;
}
static BOOL fullscreenSelectionMatches(PTTTarget *target) {
    // AppKit hides the native tab group in fullscreen. Only accept the original
    // selected window/input pair; never infer a replacement from a title or index.
    id fullscreen=attribute(target.window,CFSTR("AXFullScreen"));
    return [fullscreen isKindOfClass:NSNumber.class] && [fullscreen boolValue]
        && windowTabs(target.window).count==0
        && same(attribute(target.application,kAXFocusedWindowAttribute),target.window)
        && same(attribute(target.application,kAXFocusedUIElementAttribute),target.element);
}
static BOOL valid(PTTTarget *target) {
    if (!target || [NSRunningApplication runningApplicationWithProcessIdentifier:target.pid].terminated) return NO;
    id windows=attribute(target.application,kAXWindowsAttribute);
    if (![windows isKindOfClass:NSArray.class]) return NO;
    if (target.tab) {
        if (windowContainingTab(windows,target.tab)) return YES;
        if (!fullscreenSelectionMatches(target)) return NO;
    }
    for (id window in windows) if (same(window,target.window)) return YES;
    return NO;
}
static BOOL focused(PTTTarget *target) {
    if (!valid(target)) return NO;
    id system=CFBridgingRelease(AXUIElementCreateSystemWide());
    AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)system,0.2);
    id app=attribute(system,kAXFocusedApplicationAttribute);
    return same(app,target.application)
        && same(attribute(app,kAXFocusedWindowAttribute),target.window)
        && (!target.tab || same(selectedWindowTab(target.window),target.tab) || fullscreenSelectionMatches(target))
        && same(attribute(app,kAXFocusedUIElementAttribute),target.element);
}

uint64_t ptt_capture_target(void) {
    if (!AXIsProcessTrusted()) return 0;
    initializeTargets();
    __block uint64_t token=0;
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        id system=CFBridgingRelease(AXUIElementCreateSystemWide());
        AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)system,0.2);
        id app=attribute(system,kAXFocusedApplicationAttribute);
        if (!app) return;
        AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)app,0.2);
        pid_t pid=0;
        if (AXUIElementGetPid((__bridge AXUIElementRef)app,&pid)!=kAXErrorSuccess || pid==getpid()) return;
        id window=attribute(app,kAXFocusedWindowAttribute);
        id element=attribute(app,kAXFocusedUIElementAttribute);
        if (!window || !element || secure(element)) return;
        id tab=selectedWindowTab(window);
        AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)window,0.2);
        AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)element,0.2);
        for (NSNumber *key in targets) {
            PTTTarget *existing=targets[key];
            if (same(existing.window,window) && same(existing.element,element) && existing.pid==pid
                && ((!existing.tab && !tab) || same(existing.tab,tab))) {
                token=key.unsignedLongLongValue; return;
            }
        }
        // Bounded retention; stale sessions safely fall back to manual copy.
        if (targets.count>=128) {
            NSNumber *oldest=[[targets allKeys] sortedArrayUsingSelector:@selector(compare:)].firstObject;
            [targets removeObjectForKey:oldest];
        }
        PTTTarget *target=[PTTTarget new];
        target.pid=pid; target.application=app; target.window=window; target.element=element; target.tab=tab;
        token=nextTarget++;
        targets[@(token)]=target;
    }});
    return token;
}
bool ptt_target_valid(uint64_t token) {
    initializeTargets(); __block BOOL result=NO;
    dispatch_sync(targetQueue, ^{ @autoreleasepool { result=valid(targets[@(token)]); }});
    return result;
}
bool ptt_target_focused(uint64_t token) {
    initializeTargets(); __block BOOL result=NO;
    dispatch_sync(targetQueue, ^{ @autoreleasepool { result=focused(targets[@(token)]); }});
    return result;
}
static void restoreNativeTarget(PTTTarget *target) {
    target.restoreRequested=YES;
    if (target.tab) {
        target.tabError=AXUIElementPerformAction((__bridge AXUIElementRef)target.tab,kAXPressAction);
        if (target.tabError!=kAXErrorSuccess) return;
    }
    // AppKit activation is a request, not proof that this background app can transfer focus.
    // AX activation uses the already granted accessibility capability. Rust still verifies all identities.
    target.frontmostError=AXUIElementSetAttributeValue((__bridge AXUIElementRef)target.application,kAXFrontmostAttribute,kCFBooleanTrue);
    target.raiseError=AXUIElementPerformAction((__bridge AXUIElementRef)target.window,kAXRaiseAction);
    target.mainError=AXUIElementSetAttributeValue((__bridge AXUIElementRef)target.window,kAXMainAttribute,kCFBooleanTrue);
    target.focusError=AXUIElementSetAttributeValue((__bridge AXUIElementRef)target.element,kAXFocusedAttribute,kCFBooleanTrue);
}
bool ptt_restore_target(uint64_t token) {
    initializeTargets(); __block PTTTarget *target;
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        PTTTarget *candidate=targets[@(token)];
        if (valid(candidate)) target=candidate;
    }});
    if (!target) return false;
    // Workspace activates the existing installation from a background dictation app.
    // Cooperative activation cannot transfer focus from us when we are not active.
    CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+1.2;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSRunningApplication *application=[NSRunningApplication runningApplicationWithProcessIdentifier:target.pid];
        if (!application || application.terminated || !application.bundleURL || CFAbsoluteTimeGetCurrent()>deadline) return;
        NSWorkspaceOpenConfiguration *configuration=NSWorkspaceOpenConfiguration.configuration;
        configuration.activates=YES; configuration.addsToRecentItems=NO; configuration.promptsUserIfNeeded=NO;
        configuration.createsNewApplicationInstance=NO; configuration.allowsRunningApplicationSubstitution=NO;
        [NSWorkspace.sharedWorkspace openApplicationAtURL:application.bundleURL configuration:configuration
              completionHandler:^(NSRunningApplication *opened,NSError *error) {
            dispatch_async(targetQueue, ^{ @autoreleasepool {
                if (error || !opened || opened.processIdentifier!=target.pid) { target.activationResult=@"目标实例不可用"; return; }
                if (CFAbsoluteTimeGetCurrent()>deadline) { target.activationResult=@"激活超时"; return; }
                if (!valid(target)) { target.activationResult=@"目标已关闭"; return; }
                target.activationResult=@"已请求";
                restoreNativeTarget(target);
            }});
        }];
    });
    return true;
}
char *ptt_read_text(uint64_t token) {
    initializeTargets(); __block char *result=NULL;
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        PTTTarget *target=targets[@(token)];
        if (!focused(target) || secure(target.element)) return;
        id value=attribute(target.element,kAXValueAttribute);
        if ([value isKindOfClass:NSString.class]) result=strdup([value UTF8String]);
    }});
    return result;
}
void ptt_free_string(char *value) { free(value); }
bool ptt_key_down(uint16_t code) { return CGEventSourceKeyState(kCGEventSourceStateHIDSystemState,code); }

// Owned exclusively by the macos-hotkeys thread and its CFRunLoop callback.
// Queue complete states so even a down/up pair between Rust polls retains both edges.
static CFMachPortRef keyboardTap;
static uint8_t keyboardState[128], keyboardQueue[256][128];
static unsigned keyboardHead, keyboardCount;
static bool keyboardReset;
static CFAbsoluteTime keyboardReconciled;
static void readKeyboardState(void) {
    for (unsigned i=0;i<128;i++) keyboardState[i]=CGEventSourceKeyState(kCGEventSourceStateCombinedSessionState,i);
}
static void queueKeyboardState(void) {
    if (keyboardReset) return;
    if (keyboardCount==256) {
        // A stalled consumer must not replay stale recording starts.
        keyboardHead=keyboardCount=0; keyboardReset=true; readKeyboardState();
        return;
    }
    memcpy(keyboardQueue[(keyboardHead+keyboardCount)%256],keyboardState,128);
    keyboardCount++;
}
static CGEventRef observeKeyboard(CGEventTapProxy proxy,CGEventType type,CGEventRef event,void *context) {
    (void)proxy; (void)context;
    if (type==kCGEventTapDisabledByTimeout || type==kCGEventTapDisabledByUserInput) {
        keyboardHead=keyboardCount=0; keyboardReset=true; readKeyboardState();
        CGEventTapEnable(keyboardTap,true);
        return event;
    }
    if (CGEventGetIntegerValueField(event,kCGKeyboardEventAutorepeat)) return event;
    uint16_t code=(uint16_t)CGEventGetIntegerValueField(event,kCGKeyboardEventKeycode);
    if (code>=128) return event;
    uint8_t old[128]; memcpy(old,keyboardState,128);
    if (type==kCGEventFlagsChanged) {
        CGEventFlags flags=CGEventGetFlags(event);
        const uint16_t codes[]={59,62,56,60,58,61,55,54,57};
        const CGEventFlags masks[]={NX_DEVICELCTLKEYMASK,NX_DEVICERCTLKEYMASK,NX_DEVICELSHIFTKEYMASK,NX_DEVICERSHIFTKEYMASK,NX_DEVICELALTKEYMASK,NX_DEVICERALTKEYMASK,NX_DEVICELCMDKEYMASK,NX_DEVICERCMDKEYMASK,kCGEventFlagMaskAlphaShift};
        for (unsigned i=0;i<9;i++) keyboardState[codes[i]]=(flags&masks[i])!=0;
    } else {
        keyboardState[code]=type==kCGEventKeyDown;
    }
    if (memcmp(old,keyboardState,128)!=0) queueKeyboardState();
    return event; // Passive tap never modifies or consumes another application's input.
}
bool ptt_hotkeys_start(void) {
    if (keyboardTap) return true;
    CGEventMask mask=CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp)|CGEventMaskBit(kCGEventFlagsChanged);
    keyboardTap=CGEventTapCreate(kCGSessionEventTap,kCGHeadInsertEventTap,kCGEventTapOptionListenOnly,mask,observeKeyboard,NULL);
    if (!keyboardTap) return false;
    CFRunLoopSourceRef source=CFMachPortCreateRunLoopSource(kCFAllocatorDefault,keyboardTap,0);
    if (!source) { CFRelease(keyboardTap); keyboardTap=NULL; return false; }
    CFRunLoopAddSource(CFRunLoopGetCurrent(),source,kCFRunLoopDefaultMode);
    CFRelease(source);
    readKeyboardState(); keyboardReset=true; keyboardReconciled=CFAbsoluteTimeGetCurrent();
    CGEventTapEnable(keyboardTap,true);
    return true;
}
int32_t ptt_hotkeys_next(uint8_t *down) {
    if (!keyboardTap) return -1;
    if (!keyboardCount && !keyboardReset) CFRunLoopRunInMode(kCFRunLoopDefaultMode,0.016,true);
    if (keyboardReset) {
        keyboardReset=false; memcpy(down,keyboardState,128); return 2;
    }
    if (keyboardCount) {
        memcpy(down,keyboardQueue[keyboardHead],128);
        keyboardHead=(keyboardHead+1)%256; keyboardCount--; return 1;
    }
    // Recover a lost key-up / sleep-wake transition without manufacturing key events.
    if (CFAbsoluteTimeGetCurrent()-keyboardReconciled>=0.5) {
        readKeyboardState(); keyboardReconciled=CFAbsoluteTimeGetCurrent();
    }
    memcpy(down,keyboardState,128); return 0;
}
bool ptt_accessibility(void) { return AXIsProcessTrusted(); }
bool ptt_input_monitoring(void) { return CGPreflightListenEventAccess(); }
int ptt_microphone(void) { return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]; }
void ptt_request_permission(int kind) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (kind==0) {
            if ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]==AVAuthorizationStatusNotDetermined)
                [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) { (void)granted; }];
            else [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"]];
        } else if (kind==1) {
            NSDictionary *options=@{(__bridge NSString *)kAXTrustedCheckOptionPrompt:@YES};
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
            [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"]];
        } else if (kind==2) {
            CGRequestListenEventAccess();
            [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"]];
        }
    });
}
bool ptt_send_shortcut(uint16_t code) {
    if (!AXIsProcessTrusted() || !CGPreflightPostEventAccess()) return false;
    CGEventRef down=CGEventCreateKeyboardEvent(NULL,code,true);
    CGEventRef up=CGEventCreateKeyboardEvent(NULL,code,false);
    if (!down || !up) { if (down) CFRelease(down); if (up) CFRelease(up); return false; }
    CGEventSetFlags(down,kCGEventFlagMaskCommand);
    CGEventSetFlags(up,0);
    CGEventPost(kCGSessionEventTap,down);
    usleep(15000);
    CGEventPost(kCGSessionEventTap,up);
    CFRelease(down); CFRelease(up);
    return true;
}
// Only exposed by the opt-in ATDD Rust module. No text, titles or clipboard data.
static NSString *atddFixtureApplication(NSString *path) {
    if ([path.pathExtension isEqualToString:@"txt"]) return @"com.apple.TextEdit";
    if ([path.pathExtension isEqualToString:@"html"]) return @"com.google.Chrome";
    return nil;
}
static bool atddFixtureMatches(NSString *bundle, NSString *document, NSString *role, NSString *path) {
    if (![bundle isEqualToString:atddFixtureApplication(path)] || ![role isEqualToString:@"AXTextArea"]) return false;
    NSURL *url=document ? [NSURL URLWithString:document] : nil;
    if (!url.isFileURL || (url.host.length && ![url.host isEqualToString:@"localhost"])) return false;
    NSString *actual=[url.path stringByResolvingSymlinksInPath];
    return [actual isEqualToString:[path stringByResolvingSymlinksInPath]];
}
// Test setup only: open a file created by the opt-in driver, with explicit activation.
static bool selectFixtureText(PTTTarget *target, NSString *expected, CFRange range) {
    target.fixtureSelectionStatus=@"焦点或内容不匹配";
    if (!focused(target) || ![expected isKindOfClass:NSString.class] ||
        ![attribute(target.element,kAXValueAttribute) isEqual:expected]) return false;
    target.fixtureSelectionStatus=@"选区范围无效";
    if (range.location<0 || range.length<0 || (NSUInteger)range.location>expected.length ||
        (NSUInteger)range.length>expected.length-(NSUInteger)range.location) return false;
    AXValueRef value=AXValueCreate(kAXValueCFRangeType,&range);
    AXError error=AXUIElementSetAttributeValue((__bridge AXUIElementRef)target.element,kAXSelectedTextRangeAttribute,value);
    CFRelease(value);
    target.fixtureSelectionStatus=[NSString stringWithFormat:@"选区设置 AX=%d",error];
    if (error!=kAXErrorSuccess) return false;
    // Browser renderers may acknowledge AX writes before their read cache updates.
    // Write once, then verify for a bounded interval; never proceed on a stale range.
    CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+0.3;
    do {
        target.fixtureSelectionStatus=@"等待期间焦点或内容改变";
        if (!focused(target) || ![attribute(target.element,kAXValueAttribute) isEqual:expected]) return false;
        target.fixtureSelectionStatus=@"选区读回不匹配或不支持";
        id selected=attribute(target.element,kAXSelectedTextRangeAttribute);
        CFRange actual;
        if (selected && CFGetTypeID((__bridge CFTypeRef)selected)==AXValueGetTypeID() &&
            AXValueGetValue((__bridge AXValueRef)selected,kAXValueCFRangeType,&actual) &&
            actual.location==range.location && actual.length==range.length) {
            target.fixtureSelectionStatus=@"匹配";
            return focused(target);
        }
        usleep(15000);
    } while (CFAbsoluteTimeGetCurrent()<deadline);
    return false;
}
// Recording still captures the foreground target through the ordinary production callback.
bool ptt_atdd_open_fixture(const char *path) {
    NSString *filename=[NSString stringWithUTF8String:path];
    __block bool opened=false;
    dispatch_semaphore_t done=dispatch_semaphore_create(0);
    dispatch_async(dispatch_get_main_queue(), ^{
        NSWorkspace *workspace=NSWorkspace.sharedWorkspace;
        NSString *bundle=atddFixtureApplication(filename);
        NSURL *application=bundle ? [workspace URLForApplicationWithBundleIdentifier:bundle] : nil;
        if (!application) { dispatch_semaphore_signal(done); return; }
        NSWorkspaceOpenConfiguration *configuration=NSWorkspaceOpenConfiguration.configuration;
        configuration.activates=YES; configuration.addsToRecentItems=NO;
        [workspace openURLs:@[[NSURL fileURLWithPath:filename]] withApplicationAtURL:application
              configuration:configuration completionHandler:^(NSRunningApplication *app, NSError *error) {
            opened=app!=nil && error==nil; dispatch_semaphore_signal(done);
        }];
    });
    if (dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC))!=0) return false;
    return opened;
}
bool ptt_atdd_fixture_focused(uint64_t token, const char *path) {
    initializeTargets(); __block bool matches=false;
    NSString *filename=[NSString stringWithUTF8String:path];
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        PTTTarget *target=targets[@(token)];
        if (!focused(target)) return;
        matches=atddFixtureMatches([NSRunningApplication runningApplicationWithProcessIdentifier:target.pid].bundleIdentifier,
                                  attribute(target.window,kAXDocumentAttribute),attribute(target.element,kAXRoleAttribute),filename);
    }});
    return matches;
}
bool ptt_atdd_select_fixture(uint64_t token, const char *path, const char *expected, uint64_t start, uint64_t length) {
    initializeTargets(); __block bool selected=false;
    NSString *filename=[NSString stringWithUTF8String:path];
    NSString *text=[NSString stringWithUTF8String:expected];
    if (start>LONG_MAX || length>LONG_MAX) return false;
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        PTTTarget *target=targets[@(token)];
        if (!target || !atddFixtureMatches([NSRunningApplication runningApplicationWithProcessIdentifier:target.pid].bundleIdentifier,
            attribute(target.window,kAXDocumentAttribute),attribute(target.element,kAXRoleAttribute),filename)) return;
        selected=selectFixtureText(target,text,CFRangeMake((CFIndex)start,(CFIndex)length));
    }});
    return selected;
}
// Opt-in diagnostics: role metadata only, with no descent into document content.
static NSString *atddRoleName(id value) {
    if (![value isKindOfClass:NSString.class] || [value length]>48 || ![value hasPrefix:@"AX"]) return @"-";
    NSCharacterSet *letters=NSCharacterSet.letterCharacterSet;
    return [value rangeOfCharacterFromSet:letters.invertedSet].location==NSNotFound ? value : @"-";
}
static void atddDescribeNode(id node, NSUInteger depth, NSUInteger *remaining,
                             CFAbsoluteTime deadline, NSMutableArray *lines) {
    if (!node || depth>12 || *remaining==0 || CFAbsoluteTimeGetCurrent()>deadline) return;
    (*remaining)--;
    AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)node,0.05);
    NSString *role=atddRoleName(attribute(node,kAXRoleAttribute));
    NSString *subrole=atddRoleName(attribute(node,kAXSubroleAttribute));
    if ([@[@"AXWebArea",@"AXTextArea",@"AXTextField",@"AXStaticText"] containsObject:role] ||
        [subrole isEqualToString:@"AXSecureTextField"]) {
        [lines addObject:[NSString stringWithFormat:@"%lu:%@/%@ skip",(unsigned long)depth,role,subrole]];
        return;
    }
    id children=attribute(node,kAXChildrenAttribute);
    id tabs=[role isEqualToString:@"AXTabGroup"] ? attribute(node,kAXTabsAttribute) : nil;
    [lines addObject:[NSString stringWithFormat:@"%lu:%@/%@ children=%ld tabs=%ld",(unsigned long)depth,role,subrole,
        [children isKindOfClass:NSArray.class] ? (long)[children count] : -1L,
        [tabs isKindOfClass:NSArray.class] ? (long)[tabs count] : -1L]];
    if (![children isKindOfClass:NSArray.class]) return;
    for (id child in [children subarrayWithRange:NSMakeRange(0,MIN([children count],80))]) {
        if (*remaining==0 || CFAbsoluteTimeGetCurrent()>deadline) break;
        atddDescribeNode(child,depth+1,remaining,deadline,lines);
    }
}
char *ptt_atdd_target_description(uint64_t token) {
    initializeTargets(); __block NSString *description=@"未捕获目标";
    dispatch_sync(targetQueue, ^{ @autoreleasepool {
        PTTTarget *target=targets[@(token)];
        if (!target) return;
        NSString *bundle=[NSRunningApplication runningApplicationWithProcessIdentifier:target.pid].bundleIdentifier ?: @"unknown";
        NSString *role=attribute(target.element,kAXRoleAttribute) ?: @"unknown";
        id system=CFBridgingRelease(AXUIElementCreateSystemWide());
        AXUIElementSetMessagingTimeout((__bridge AXUIElementRef)system,0.2);
        id active=attribute(system,kAXFocusedApplicationAttribute);
        description=[NSString stringWithFormat:@"%@ / %@；有效=%d；焦点(应用/窗口/输入框)=%d/%d/%d；恢复=%d；Workspace=%@；标签=%d/%d；AX返回(前台/抬窗/主窗/焦点)=%d/%d/%d/%d",
                     bundle,role,valid(target),same(active,target.application),same(attribute(active,kAXFocusedWindowAttribute),target.window),
                     same(attribute(active,kAXFocusedUIElementAttribute),target.element),target.restoreRequested,
                     target.activationResult?:@"未请求",target.tab!=nil,target.tabError,
                     target.frontmostError,target.raiseError,target.mainError,target.focusError];
        if (target.fixtureSelectionStatus) description=[description stringByAppendingFormat:@"；选区准备=%@",target.fixtureSelectionStatus];
        NSMutableArray *tree=[NSMutableArray new];
        NSUInteger remaining=80;
        atddDescribeNode(target.window,0,&remaining,CFAbsoluteTimeGetCurrent()+0.5,tree);
        description=[description stringByAppendingFormat:@"；原生结构：\n%@",[tree componentsJoinedByString:@"\n"]];
    }});
    return strdup([[NSString stringWithFormat:@"%@；PostEvent=%@",description,CGPreflightPostEventAccess()?@"允许":@"拒绝"] UTF8String]);
}
void ptt_configure_overlay(void *pointer) {
    NSWindow *window=(__bridge NSWindow *)pointer;
    window.collectionBehavior=NSWindowCollectionBehaviorCanJoinAllSpaces|NSWindowCollectionBehaviorFullScreenAuxiliary;
    window.level=NSFloatingWindowLevel;
    window.hidesOnDeactivate=NO;
}

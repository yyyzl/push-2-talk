#import <AppKit/AppKit.h>

// A snapshot owns bytes, not live pasteboard items or deferred data providers.
// Each session is called serially by the Rust clipboard transaction guard.
@interface PTTClipboardSession : NSObject
@property NSPasteboard *board;
@property NSArray<NSDictionary<NSPasteboardType, NSData *> *> *items;
@property NSInteger revision;
@property BOOL modified;
@property BOOL finished;
@end
@implementation PTTClipboardSession
@end

static PTTClipboardSession *ptt_clipboard_capture(NSPasteboard *board, NSUInteger limit) {
    if (!board) return nil;
    NSInteger revision = board.changeCount;
    NSArray<NSPasteboardItem *> *source = board.pasteboardItems;
    if (!source || source.count > 256) return nil;
    NSMutableArray *items = [NSMutableArray new];
    NSUInteger size = 0;
    for (NSPasteboardItem *item in source) {
        NSArray<NSPasteboardType> *types = item.types;
        if (types.count == 0 || types.count > 128) return nil;
        NSMutableDictionary *values = [NSMutableDictionary new];
        for (NSPasteboardType type in types) {
            NSData *data = [item dataForType:type];
            // Refuse the transaction before clearing anything if any format
            // cannot be materialized, or the complete snapshot exceeds its cap.
            if (!data || data.length > limit - size) return nil;
            size += data.length;
            values[type] = [data copy];
        }
        [items addObject:[values copy]];
    }
    if (board.changeCount != revision) return nil;
    PTTClipboardSession *session = [PTTClipboardSession new];
    session.board = board;
    session.items = [items copy];
    session.revision = revision;
    return session;
}

static BOOL ptt_clipboard_is_owned(PTTClipboardSession *session) {
    return session && !session.finished && session.board.changeCount == session.revision;
}

// Status: 0 success, 1 a newer copy won, 2 native failure.
static int ptt_clipboard_write(PTTClipboardSession *session, NSString *text) {
    if (!ptt_clipboard_is_owned(session)) return 1;
    session.revision = [session.board clearContents];
    session.modified = YES;
    BOOL written = [session.board setString:text forType:NSPasteboardTypeString];
    // Keep the revision returned by clearContents: adopting a later revision
    // here could incorrectly claim a concurrent user's copy as our own.
    if (session.board.changeCount != session.revision) return 1;
    return written ? 0 : 2;
}

static int ptt_clipboard_claim_selection(PTTClipboardSession *session, NSString *expected) {
    if (!session || session.finished || !session.modified || expected.length == 0) return 1;
    NSInteger revision = session.board.changeCount;
    NSString *actual = [session.board.pasteboardItems.firstObject stringForType:NSPasteboardTypeString];
    if (revision == session.revision || ![actual isEqualToString:expected] ||
        session.board.changeCount != revision) return 1;
    session.revision = revision;
    return 0;
}

static int ptt_clipboard_restore_session(PTTClipboardSession *session) {
    if (!session || session.finished) return 0;
    BOOL restore = session.modified && ptt_clipboard_is_owned(session);
    session.finished = YES; // A manual restore followed by Drop cannot write twice.
    if (!restore) return 0;
    NSMutableArray<NSPasteboardItem *> *items = [NSMutableArray new];
    for (NSDictionary<NSPasteboardType, NSData *> *values in session.items) {
        NSPasteboardItem *item = [NSPasteboardItem new];
        for (NSPasteboardType type in values) {
            if (![item setData:values[type] forType:type]) return 2;
        }
        [items addObject:item];
    }
    // Recheck after constructing the replacement. NSPasteboard has no atomic
    // compare-and-swap; the unavoidable final check/write gap is kept short.
    if (session.board.changeCount != session.revision) return 0;
    [session.board clearContents];
    return items.count == 0 || [session.board writeObjects:items] ? 0 : 2;
}

void *ptt_clipboard_begin(void) {
    @autoreleasepool { @try {
        PTTClipboardSession *session = ptt_clipboard_capture(NSPasteboard.generalPasteboard, 64 * 1024 * 1024);
        return session ? (__bridge_retained void *)session : NULL;
    } @catch (NSException *exception) { return NULL; } }
}

int ptt_clipboard_set_text(void *handle, const unsigned char *bytes, size_t length) {
    @autoreleasepool { @try {
        NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
        return text ? ptt_clipboard_write((__bridge PTTClipboardSession *)handle, text) : 2;
    } @catch (NSException *exception) { return 2; } }
}

int ptt_clipboard_claim_copy(void *handle, const unsigned char *bytes, size_t length) {
    @autoreleasepool { @try {
        NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
        return text ? ptt_clipboard_claim_selection((__bridge PTTClipboardSession *)handle, text) : 2;
    } @catch (NSException *exception) { return 2; } }
}

int ptt_clipboard_owned(void *handle) {
    @autoreleasepool { @try {
        return ptt_clipboard_is_owned((__bridge PTTClipboardSession *)handle) ? 0 : 1;
    } @catch (NSException *exception) { return 2; } }
}

int ptt_clipboard_restore(void *handle) {
    @autoreleasepool { @try {
        return ptt_clipboard_restore_session((__bridge PTTClipboardSession *)handle);
    } @catch (NSException *exception) { return 2; } }
}

void ptt_clipboard_dispose(void *handle) {
    @autoreleasepool { id session = (__bridge_transfer id)handle; (void)session; }
}

// Private pasteboards only: these tests must never replace the user's clipboard.
#import <AppKit/AppKit.h>
#ifndef PTT_FIXTURE_ONLY
#include "../src/platform/macos/clipboard.m"
#endif

static int checks = 0;
#define CHECK(condition) do { checks++; if (!(condition)) { \
    fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #condition); exit(1); \
} } while (0)

static NSArray<NSPasteboardItem *> *richFixture(void) {
    NSPasteboardItem *rich = [NSPasteboardItem new];
    [rich setString:@"PTT_RICH" forType:NSPasteboardTypeString];
    [rich setData:[@"{\\rtf1\\ansi\\b PTT_RICH}" dataUsingEncoding:NSUTF8StringEncoding] forType:NSPasteboardTypeRTF];
    // A real PNG representation, plus arbitrary app-specific metadata.
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL
        pixelsWide:1 pixelsHigh:1 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES
        isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:4 bitsPerPixel:32];
    [rich setData:[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] forType:NSPasteboardTypePNG];
    [rich setData:[NSData data] forType:@"org.pushtotalk.test.empty-metadata"];
    NSPasteboardItem *file = [NSPasteboardItem new];
    [file setString:@"file:///tmp/ptt-test.txt" forType:NSPasteboardTypeFileURL];
    return @[rich, file];
}

static NSArray *contents(NSPasteboard *board) {
    NSMutableArray *items = [NSMutableArray new];
    for (NSPasteboardItem *item in board.pasteboardItems) {
        NSMutableDictionary *data = [NSMutableDictionary new];
        for (NSString *type in item.types) data[type] = [item dataForType:type];
        [items addObject:data];
    }
    return items;
}

int main(void) {
    @autoreleasepool {
        NSPasteboard *board = [NSPasteboard pasteboardWithUniqueName];
        CHECK([board writeObjects:richFixture()]);
        NSArray *original = contents(board);
        CHECK(original.count == 2);
        CHECK([original[0][NSPasteboardTypePNG] length] > 0);
        CHECK([original[0][NSPasteboardTypeRTF] length] > 0);
#ifndef PTT_FIXTURE_ONLY
        NSInteger declared = [board clearContents];
        CHECK([board setString:@"probe" forType:NSPasteboardTypeString]);
        CHECK(board.changeCount == declared); // Writing declared types does not acquire a new revision.
        [board clearContents];
        CHECK([board writeObjects:richFixture()]);
        // The same implementation used by the Rust bridge, on an isolated board.
        PTTClipboardSession *session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(session != nil);
        CHECK(ptt_clipboard_write(session, @"dictated text") == 0);
        CHECK([[board stringForType:NSPasteboardTypeString] isEqual:@"dictated text"]);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([contents(board) isEqual:original]);
        NSInteger restored = board.changeCount;
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK(board.changeCount == restored); // Explicit restore + Drop is idempotent.

        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"result") == 0);
        [board clearContents];
        [board setString:@"user copied something newer" forType:NSPasteboardTypeString];
        CHECK(!ptt_clipboard_is_owned(session));
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([[board stringForType:NSPasteboardTypeString] isEqual:@"user copied something newer"]);

        // A newer copy even before our initial write must not be overwritten.
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        [board clearContents];
        [board setString:@"new before write" forType:NSPasteboardTypeString];
        CHECK(ptt_clipboard_write(session, @"result") == 1);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([[board stringForType:NSPasteboardTypeString] isEqual:@"new before write"]);

        // Empty clipboard is a snapshot too.
        [board clearContents];
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"temporary") == 0);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK(board.pasteboardItems.count == 0);

        // Image-only data survives; there is no plain text fallback to hide a bug.
        NSPasteboardItem *image = [NSPasteboardItem new];
        [image setData:original[0][NSPasteboardTypePNG] forType:NSPasteboardTypePNG];
        CHECK([board writeObjects:@[image]]);
        NSArray *imageOnly = contents(board);
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"temporary") == 0);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([contents(board) isEqual:imageOnly]);

        // Dropping an unused snapshot is read-only, and a newer copy with the
        // same text still belongs to the user (content equality is insufficient).
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        NSInteger unchanged = board.changeCount;
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK(board.changeCount == unchanged);
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"same text") == 0);
        [board clearContents];
        [board setString:@"same text" forType:NSPasteboardTypeString];
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([[board stringForType:NSPasteboardTypeString] isEqual:@"same text"]);
        [board clearContents];
        NSPasteboardItem *imageAgain = [NSPasteboardItem new];
        [imageAgain setData:original[0][NSPasteboardTypePNG] forType:NSPasteboardTypePNG];
        CHECK([board writeObjects:@[imageAgain]]);

        // Snapshot refusal must leave data intact, not preserve a partial copy.
        CHECK(ptt_clipboard_capture(board, 1) == nil);
        CHECK([contents(board) isEqual:imageOnly]);

        // Assistant selection changes the board via the target app's Cmd+C.
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"") == 0);
        [board clearContents];
        [board setString:@"selected words" forType:NSPasteboardTypeString];
        CHECK(ptt_clipboard_claim_selection(session, @"different words") == 1);
        CHECK(ptt_clipboard_claim_selection(session, @"selected words") == 0);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([contents(board) isEqual:imageOnly]);

        // Failed/no selection restores the temporary empty sentinel normally.
        session = ptt_clipboard_capture(board, 64 * 1024 * 1024);
        CHECK(ptt_clipboard_write(session, @"") == 0);
        CHECK(ptt_clipboard_claim_selection(session, @"") == 1);
        CHECK(ptt_clipboard_restore_session(session) == 0);
        CHECK([contents(board) isEqual:imageOnly]);
#endif
        [board releaseGlobally];
        printf("PASS: %d private pasteboard assertions\n", checks);
    }
    return 0;
}

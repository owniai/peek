// edge.m — language-specific edge cases

// ── Category implementation ──
@implementation Vehicle (Maintenance)
- (void)performMaintenance {
    NSLog(@"Maintenance");
}
@end

// ── Protocol with @optional ──
@protocol Serializable
- (void)serialize;
@optional
- (void)deserialize;
@end

// ── Typedef with struct containing fields ──
typedef struct {
    float width;
    float height;
} Size;

// ── Implementation without corresponding interface ──
@implementation Standalone
- (void)doWork {
}
@end

// ── Interface without superclass ──
@interface BaseClass
- (void)initialize;
@end

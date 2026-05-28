// core.m — all kind classifications, scope paths, signature formats, nested scope

// ── Class interface with ivars, properties, method declarations ──
@interface Vehicle : NSObject {
    NSString *_model;
    NSInteger _year;
}
@property (nonatomic, strong) NSString *model;
@property (nonatomic, assign) NSInteger year;
- (void)startEngine;
+ (id)defaultVehicle;
@end

// ── Extension (empty category) ──
@interface Vehicle ()
- (void)privateSetup;
@end

// ── Named category ──
@interface Vehicle (Maintenance)
- (void)performMaintenance;
@end

// ── Class implementation ──
@implementation Vehicle
- (void)startEngine {
    NSLog(@"Starting %@", _model);
}
+ (id)defaultVehicle {
    return nil;
}
@end

// ── Protocol ──
@protocol Drivable
- (void)drive;
@property (nonatomic, strong) NSString *destination;
@end

// ── Forward declaration ──
@class Person;

// ── NS_ENUM ──
typedef NS_ENUM(NSInteger, Direction) {
    DirectionNorth,
    DirectionSouth,
};

// ── C function definition ──
int computeLength(void) {
    return 0;
}

// ── C function prototype ──
void logMessage(void);

// ── extern const ──
extern const int kMaxRetries;

// ── #define macro ──
#define VERSION 2

// ── typedef struct ──
typedef struct { double x; double y; } Point;
